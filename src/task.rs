//! Uzun süren işlemlerin arka planda çalıştırılması.
//!
//! Python sürümü `QThreadPool` + Qt sinyalleri kullanıyordu. Burada karşılığı
//! `std::thread` + bir `mpsc` kanalı + paylaşılan bir iptal bayrağı.
//!
//! Bu modül bilerek `egui` içermez — uyandırma bir geri çağırmanın arkasında.
//! Böylece hem katman kuralı korunur hem de eşzamanlılık mantığı pencere
//! açmadan test edilebilir.

use crate::core::OpError;
use crate::core::ops::ProgressSink;
use std::cell::Cell;
use std::panic::AssertUnwindSafe;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::time::{Duration, Instant};

/// İki ilerleme bildirimi arasındaki en kısa süre.
///
/// Bir save on binlerce dosya içerebilir. Her dosyada kanal mesajı gönderip
/// yeniden çizim istemek, işi ilerletmekten çok çizime harcanan bir döngü
/// yaratır. ~30 fps gözle görülen ilerleme için fazlasıyla yeterli.
const PROGRESS_INTERVAL: Duration = Duration::from_millis(33);

/// İş parçacığı hiç haber vermeden öldüğünde kullanılan mesaj.
const WORKER_DIED: &str = "arka plan işlemi beklenmedik şekilde sonlandı";

/// Arayüzü yeniden çizmeye çağıran geri çağırma.
///
/// Üretimde `ctx.request_repaint()`, testlerde boş bir kapanış.
pub type Wake = Arc<dyn Fn() + Send + Sync>;

/// Hangi işlem çalışıyor. Arayüz başarı/hata metnini buna göre seçer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskKind {
    Backup,
    Delete,
    Export,
    Import,
    Restore,
}

/// Çalışan iş parçacığından arayüze giden mesajlar.
#[derive(Debug, Clone, PartialEq, Eq)]
enum TaskEvent {
    Progress { done: u64, total: u64 },
    Finished,
    Cancelled,
    Failed(String),
}

/// Bir işlemin nihai sonucu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Success(TaskKind),
    Cancelled,
    Error(String),
}

/// Çalışan iş parçacığına verilen [`ProgressSink`].
struct ChannelSink {
    sender: Sender<TaskEvent>,
    cancel: Arc<AtomicBool>,
    wake: Wake,
    last_sent: Cell<Instant>,
}

impl ProgressSink for ChannelSink {
    fn tick(&self, done: u64, total: u64) -> Result<(), OpError> {
        // İptal kontrolü her çağrıda: atomik okuma ucuz, gecikmeli iptal değil.
        if self.cancel.load(Ordering::Relaxed) {
            return Err(OpError::Cancelled);
        }

        // Bildirim kısılır. Son adım her zaman gönderilir ki ilerleme çubuğu
        // %100'e ulaşmadan kaybolmasın.
        if done == total || self.last_sent.get().elapsed() >= PROGRESS_INTERVAL {
            let _ = self.sender.send(TaskEvent::Progress { done, total });
            (self.wake)();
            self.last_sent.set(Instant::now());
        }
        Ok(())
    }
}

/// Çalışan bir işleme açılan pencere.
pub struct TaskHandle {
    receiver: Receiver<TaskEvent>,
    cancel: Arc<AtomicBool>,
    kind: TaskKind,
    cancellable: bool,
    done: u64,
    total: u64,
    /// En az bir ilerleme mesajı geldi mi. Bkz. [`TaskHandle::fraction`].
    reported: bool,
    settled: bool,
}

impl TaskHandle {
    pub fn kind(&self) -> TaskKind {
        self.kind
    }

    /// Silme işlemi iptal edilemez; arayüz iptal düğmesini buna göre çizer.
    pub fn is_cancellable(&self) -> bool {
        self.cancellable
    }

    pub fn cancel(&self) {
        if self.cancellable {
            self.cancel.store(true, Ordering::Relaxed);
        }
    }

    pub fn progress(&self) -> (u64, u64) {
        (self.done, self.total)
    }

    /// İlerleme çubuğu için 0.0–1.0 arası oran; henüz bilinmiyorsa `None`.
    ///
    /// `None` ile `Some(1.0)` ayrımı önemli. Tek bir `f32` döndürüldüğünde,
    /// ilk ilerleme mesajı gelmeden önce `total` sıfır oluyor ve "boş save,
    /// yani bitti" kuralı devreye girip çubuğu daha işin başında **%100**
    /// gösteriyordu. Arayüz `None` gördüğünde belirsiz bir gösterge çizer.
    ///
    /// Toplam gerçekten sıfırsa (boş bir save) 1.0 döner — Python'un
    /// `int(done / total * 100) if total else 100` ifadesiyle aynı karar.
    pub fn fraction(&self) -> Option<f32> {
        if !self.reported {
            return None;
        }
        if self.total == 0 {
            return Some(1.0);
        }
        Some((self.done as f32 / self.total as f32).clamp(0.0, 1.0))
    }

    /// Kanalı boşaltır; işlem bittiyse sonucunu döndürür.
    ///
    /// Her karede çağrılmak üzere tasarlandı: bloklamaz.
    pub fn poll(&mut self) -> Option<Outcome> {
        if self.settled {
            return None;
        }
        loop {
            match self.receiver.try_recv() {
                Ok(TaskEvent::Progress { done, total }) => {
                    self.done = done;
                    self.total = total;
                    self.reported = true;
                }
                Ok(TaskEvent::Finished) => return self.settle(Outcome::Success(self.kind)),
                Ok(TaskEvent::Cancelled) => return self.settle(Outcome::Cancelled),
                Ok(TaskEvent::Failed(message)) => return self.settle(Outcome::Error(message)),
                Err(TryRecvError::Empty) => return None,
                // İş parçacığı tek bir mesaj göndermeden öldü. Bu dal olmasa
                // ilerleme penceresi sonsuza kadar açık kalır ve uygulama
                // kilitlenmiş görünür. Python'da Qt istisnayı yakalayıp `failed`
                // sinyaline çevirdiği için bu durum ortaya çıkmıyordu.
                Err(TryRecvError::Disconnected) => {
                    return self.settle(Outcome::Error(WORKER_DIED.to_string()));
                }
            }
        }
    }

    fn settle(&mut self, outcome: Outcome) -> Option<Outcome> {
        self.settled = true;
        Some(outcome)
    }
}

/// Bir işlemi arka planda başlatır.
pub fn spawn<F>(kind: TaskKind, cancellable: bool, wake: Wake, work: F) -> TaskHandle
where
    F: FnOnce(&dyn ProgressSink) -> Result<(), OpError> + Send + 'static,
{
    let (sender, receiver) = mpsc::channel();
    let cancel = Arc::new(AtomicBool::new(false));

    let worker_cancel = Arc::clone(&cancel);
    let worker_wake = Arc::clone(&wake);
    std::thread::spawn(move || {
        let sink = ChannelSink {
            sender: sender.clone(),
            cancel: worker_cancel,
            wake: Arc::clone(&worker_wake),
            last_sent: Cell::new(Instant::now()),
        };

        // Panik yakalanıyor ki kullanıcıya "beklenmedik şekilde sonlandı" yerine
        // gerçek mesaj gösterilebilsin. Yakalanamayan bir ölüm hâlâ mümkün;
        // onu `poll` içindeki `Disconnected` dalı karşılar.
        let event = match std::panic::catch_unwind(AssertUnwindSafe(|| work(&sink))) {
            Ok(Ok(())) => TaskEvent::Finished,
            Ok(Err(OpError::Cancelled)) => TaskEvent::Cancelled,
            Ok(Err(error)) => TaskEvent::Failed(error.to_string()),
            Err(panic) => TaskEvent::Failed(panic_message(&panic)),
        };

        let _ = sender.send(event);
        worker_wake();
    });

    TaskHandle {
        receiver,
        cancel,
        kind,
        cancellable,
        done: 0,
        total: 0,
        reported: false,
        settled: false,
    }
}

fn panic_message(panic: &Box<dyn std::any::Any + Send>) -> String {
    let detail = panic
        .downcast_ref::<&str>()
        .map(|text| (*text).to_string())
        .or_else(|| panic.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "bilinmeyen neden".to_string());
    format!("arka plan işlemi çöktü: {detail}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn silent() -> Wake {
        Arc::new(|| {})
    }

    /// Sonuç gelene kadar `poll` eder. Testlerin iş parçacığı zamanlamasına
    /// bağlı olmaması için.
    fn wait(handle: &mut TaskHandle) -> Outcome {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(outcome) = handle.poll() {
                return outcome;
            }
            assert!(Instant::now() < deadline, "işlem zamanında bitmedi");
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    #[test]
    fn successful_work_reports_its_kind() {
        let mut handle = spawn(TaskKind::Backup, true, silent(), |_sink| Ok(()));
        assert_eq!(wait(&mut handle), Outcome::Success(TaskKind::Backup));
    }

    #[test]
    fn progress_reaches_the_handle() {
        let mut handle = spawn(TaskKind::Export, true, silent(), |sink| {
            sink.tick(7, 10)?;
            sink.tick(10, 10)?;
            Ok(())
        });
        assert_eq!(wait(&mut handle), Outcome::Success(TaskKind::Export));
        // Son adım kısmaya takılmadan gönderilir.
        assert_eq!(handle.progress(), (10, 10));
    }

    #[test]
    fn failure_carries_the_message() {
        let mut handle = spawn(TaskKind::Import, true, silent(), |_sink| {
            Err(OpError::EmptyArchive)
        });
        match wait(&mut handle) {
            Outcome::Error(message) => assert!(message.contains("boş"), "{message}"),
            other => panic!("beklenen hata, gelen: {other:?}"),
        }
    }

    #[test]
    fn cancellation_stops_the_work() {
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let observed = Arc::clone(&cancel_flag);

        let mut handle = spawn(TaskKind::Backup, true, silent(), move |sink| {
            // İptal edilene kadar dön; tick her turda iptali kontrol eder.
            for step in 0..1_000_000u64 {
                sink.tick(step, 1_000_000)?;
                if step > 10 {
                    observed.store(true, Ordering::Relaxed);
                }
                std::thread::sleep(Duration::from_micros(50));
            }
            Ok(())
        });

        while !cancel_flag.load(Ordering::Relaxed) {
            handle.poll();
            std::thread::sleep(Duration::from_millis(1));
        }
        handle.cancel();

        assert_eq!(wait(&mut handle), Outcome::Cancelled);
    }

    #[test]
    fn a_non_cancellable_task_ignores_cancel() {
        let mut handle = spawn(TaskKind::Delete, false, silent(), |sink| {
            sink.tick(1, 1)?;
            Ok(())
        });
        handle.cancel();
        assert!(!handle.cancel.load(Ordering::Relaxed));
        assert_eq!(wait(&mut handle), Outcome::Success(TaskKind::Delete));
    }

    #[test]
    fn a_panicking_task_surfaces_as_an_error_rather_than_hanging() {
        let mut handle = spawn(TaskKind::Backup, true, silent(), |_sink| {
            panic!("çekirdekte bir hata");
        });
        match wait(&mut handle) {
            Outcome::Error(message) => {
                assert!(message.contains("çekirdekte bir hata"), "{message}")
            }
            other => panic!("beklenen hata, gelen: {other:?}"),
        }
    }

    #[test]
    fn a_silent_death_surfaces_as_an_error() {
        // Kanalı hiç mesaj gönderilmeden düşürerek iş parçacığının haber vermeden
        // ölmesini taklit eder. Bu dal olmasa arayüz sonsuza kadar bekler.
        let (sender, receiver) = mpsc::channel();
        drop(sender);

        let mut handle = TaskHandle {
            receiver,
            cancel: Arc::new(AtomicBool::new(false)),
            kind: TaskKind::Import,
            cancellable: true,
            done: 0,
            total: 0,
            reported: false,
            settled: false,
        };

        assert_eq!(handle.poll(), Some(Outcome::Error(WORKER_DIED.to_string())));
    }

    #[test]
    fn poll_is_idempotent_once_settled() {
        let mut handle = spawn(TaskKind::Backup, true, silent(), |_sink| Ok(()));
        assert!(wait(&mut handle) == Outcome::Success(TaskKind::Backup));
        assert_eq!(handle.poll(), None);
    }

    #[test]
    fn fraction_is_unknown_until_the_first_report() {
        // Regresyon: tek bir `f32` döndürüldüğünde ilk mesaj gelmeden `total`
        // sıfır oluyor, "boş save" kuralı devreye giriyor ve ilerleme çubuğu
        // işin daha başında %100 gösteriyordu.
        let mut handle = spawn(TaskKind::Backup, true, silent(), |_sink| Ok(()));
        assert_eq!(handle.fraction(), None);

        handle.reported = true;
        assert_eq!(handle.fraction(), Some(1.0), "boş save tamamlanmış sayılır");

        handle.done = 1;
        handle.total = 4;
        assert_eq!(handle.fraction(), Some(0.25));
    }

    #[test]
    fn the_wake_callback_fires() {
        let count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let observed = Arc::clone(&count);
        let wake: Wake = Arc::new(move || {
            observed.fetch_add(1, Ordering::Relaxed);
        });

        let mut handle = spawn(TaskKind::Backup, true, wake, |sink| {
            sink.tick(1, 1)?;
            Ok(())
        });
        wait(&mut handle);

        // En az bir kez ilerleme için, bir kez de bitiş için.
        assert!(count.load(Ordering::Relaxed) >= 2);
    }
}
