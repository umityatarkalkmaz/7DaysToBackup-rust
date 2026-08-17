//! Save klasörü işlemleri: yedekle, sil, dışa aktar, içe aktar.
//!
//! Bilerek `egui`'den arınmış: hem ekransız test edilebilsinler hem de bir
//! çalışan iş parçacığında widget'a dokunmadan koşabilsinler. Uzun süren her
//! fonksiyon bir [`ProgressSink`] alır; arayüz gerçeğini verir, testler sahtesini.

use crate::core::OpError;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Bundan fazlasına açıldığını iddia eden arşiv, kullanıcının diskini
/// doldurmasına izin verilmek yerine reddedilir. Gerçek bir save için fazlasıyla
/// cömert.
pub const MAX_EXTRACT_BYTES: u64 = 20 * 1024 * 1024 * 1024;

/// Varsayılan sıkıştırma seviyesi.
///
/// zlib'in varsayılanı 6 yerine 1: save içeriğine zaten sıkıştırılmış bölge
/// verisi hakim, seviye 6 pratikte aynı boyuttaki bir arşiv için ~%20 daha fazla
/// zaman harcıyor.
pub const DEFAULT_COMPRESSION_LEVEL: i64 = 1;

/// İlerleme bildirimi ve iptal kontrolü.
///
/// Python iki ayrı geri çağırma alıyordu (`progress` ve `is_cancelled`). Tek
/// çağrıda birleştirmek dosya başına iki yerine bir çağrı yapar ve daha önemlisi
/// "iptali kontrol etmeyi unutma" hatasını yapısal olarak imkânsız kılar: ilerleme
/// bildiren her yer aynı anda iptali de kontrol etmiş olur.
pub trait ProgressSink {
    /// İptal edilmişse [`OpError::Cancelled`] döner; çağıran `?` ile durur.
    fn tick(&self, done: u64, total: u64) -> Result<(), OpError>;
}

/// İlerlemeyi umursamayan çağıranlar için.
pub struct NoopSink;

impl ProgressSink for NoopSink {
    fn tick(&self, _done: u64, _total: u64) -> Result<(), OpError> {
        Ok(())
    }
}

/// Yedek ve dışa aktarmanın paylaştığı zaman damgası — ikisi birbirinden
/// ayrışamasın diye tek yerde.
///
/// Yerel saat kasıtlı: dosya adı kullanıcının saatini taşımalı. `chrono::Local`
/// tercih edildi çünkü `time` crate'inin `now_local()` fonksiyonu Unix'te çok
/// iş parçacıklı süreçlerde `IndeterminateOffset` ile başarısız oluyor — ve bu
/// uygulama işlemleri çalışan iş parçacıklarında koşturuyor.
/// Yedek klasörü adında save adını zaman damgasından ayıran işaret.
pub const BACKUP_MARKER: &str = "_backup_";

/// Zaman damgası biçimi. Üretim ve ayrıştırma tek yerden beslensin diye sabit.
const TIMESTAMP_FORMAT: &str = "%Y.%m.%d-%H.%M.%S";

pub fn timestamp_suffix() -> String {
    chrono::Local::now().format(TIMESTAMP_FORMAT).to_string()
}

/// Çözümlenmiş bir yedek klasörü adı.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupName {
    /// Yedeği alınan save'in adı.
    pub save: String,
    /// Yedeğin alındığı an (yerel saat).
    pub taken_at: chrono::NaiveDateTime,
    /// Aynı saniyede ikinci bir yedek alındıysa [`unique_path`]'in eklediği sayaç.
    pub counter: Option<u64>,
}

/// `<save>_backup_<zaman damgası>` biçimindeki bir klasör adını çözer.
///
/// Yedekler save'lerle aynı klasörde duruyor (oyun onları save olarak görebilsin
/// ve eski sürümlerin yedekleri öksüz kalmasın diye), dolayısıyla arayüzün
/// ikisini birbirinden ayırmasının tek yolu ad.
///
/// **Zaman damgasının ayrıştırılabilmesi şart.** Yalnızca `_backup_` aramak,
/// gerçekten `Kasım_backup_denemesi` adını taşıyan bir save'i yedek sayardı ve
/// kullanıcının save'i listeden kaybolurdu.
///
/// Ayırıcı **sondan** aranıyor: `A_backup_B_backup_<damga>` adında save `A_backup_B`'dir.
pub fn parse_backup_name(dir_name: &str) -> Option<BackupName> {
    use chrono::NaiveDateTime;

    let (save, rest) = dir_name.rsplit_once(BACKUP_MARKER)?;
    if save.is_empty() {
        return None;
    }

    // Sayaçsız hâl. Zaman damgası `_` içermediği için önce bunu denemek güvenli.
    if let Ok(taken_at) = NaiveDateTime::parse_from_str(rest, TIMESTAMP_FORMAT) {
        return Some(BackupName {
            save: save.to_owned(),
            taken_at,
            counter: None,
        });
    }

    // `<damga>_<sayaç>` hâli.
    let (stamp, tail) = rest.rsplit_once('_')?;
    let counter = tail.parse::<u64>().ok()?;
    let taken_at = NaiveDateTime::parse_from_str(stamp, TIMESTAMP_FORMAT).ok()?;
    Some(BackupName {
        save: save.to_owned(),
        taken_at,
        counter: Some(counter),
    })
}

/// Bir yolun dizin mi dosya mı olduğu. [`unique_path`] için gerekli.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathKind {
    Dir,
    File,
}

/// `path`'i, doluysa `path_2` / `path_3` ... biçiminde boş bir adı döndürür.
///
/// Zaman damgasının çözünürlüğü bir saniye, dolayısıyla aynı saniyede alınan iki
/// yedek aksi halde çakışırdı.
///
/// **Python'dan bilinçli sapma.** Orada `os.path.splitext` kullanılıyor ve o da
/// *son* noktadan bölüyor. Yedek klasörü adı `SaveA_backup_2026.08.16-14.30.00`
/// biçiminde olduğu için çakışmada `SaveA_backup_2026.08.16-14.30_2.00` gibi
/// bozuk bir ad üretiliyordu. Burada uzantı ayrıştırması yalnızca dosyalarda
/// yapılır; zip adlandırması Python ile birebir aynı kalır.
pub fn unique_path(path: impl Into<PathBuf>, kind: PathKind) -> PathBuf {
    let path = path.into();
    if !path.exists() {
        return path;
    }

    let parent = path.parent().map(Path::to_path_buf).unwrap_or_default();
    let name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();

    let (stem, extension) = match kind {
        PathKind::Dir => (name.as_str(), ""),
        // `rfind` yerine `i > 0` koşulu: ".bashrc" tümüyle isimdir, uzantı değil.
        // Python'un splitext'i de böyle davranır.
        PathKind::File => match name.rfind('.') {
            Some(index) if index > 0 => (&name[..index], &name[index..]),
            _ => (name.as_str(), ""),
        },
    };

    (2u64..)
        .map(|counter| parent.join(format!("{stem}_{counter}{extension}")))
        .find(|candidate| !candidate.exists())
        .expect("sayaç sınırsız")
}

/// Düşerken hedefi silen nöbetçi.
///
/// Python `try/except BaseException: rmtree(); raise` yazıyordu. RAII bundan
/// üstün: erken `?` dönüşünde de, panikte de çalışır ve çağrı yerinde unutulamaz.
struct CleanupGuard {
    path: PathBuf,
    kind: PathKind,
    armed: bool,
}

impl CleanupGuard {
    fn arm(path: &Path, kind: PathKind) -> Self {
        Self {
            path: path.to_path_buf(),
            kind,
            armed: true,
        }
    }

    fn disarm(mut self) {
        self.armed = false;
    }
}

impl Drop for CleanupGuard {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        // Temizlik sırasındaki hata yutulur: zaten bir başarısızlığı ele alıyoruz,
        // onun üstüne ikinci bir hata bindirmek kullanıcıya bir şey kazandırmaz.
        let _ = match self.kind {
            PathKind::Dir => fs::remove_dir_all(&self.path),
            PathKind::File => fs::remove_file(&self.path),
        };
    }
}

/// `walkdir` hatasını yol bilgisiyle `OpError`'a çevirir.
fn walk_error(root: &Path) -> impl Fn(walkdir::Error) -> OpError + use<'_> {
    move |err| {
        let path = err.path().unwrap_or(root).to_path_buf();
        OpError::Io {
            path,
            source: err
                .into_io_error()
                .unwrap_or_else(|| io::Error::other("dizin ağacında döngü var")),
        }
    }
}

/// Ağacı gezip dosyaları döndürür.
///
/// `sort_by_file_name`: ilerleme bildirimi ve testler deterministik olsun.
/// Sembolik bağlar izlenmez (walkdir varsayılanı), Python'un `os.walk`'ı gibi.
fn walk_files(root: &Path) -> Result<Vec<PathBuf>, OpError> {
    let mut found = Vec::new();
    for entry in WalkDir::new(root).sort_by_file_name() {
        let entry = entry.map_err(walk_error(root))?;
        if !entry.file_type().is_dir() && entry.depth() > 0 {
            found.push(entry.into_path());
        }
    }
    Ok(found)
}

/// `shutil.copy2` paritesi için değişiklik zamanını taşır.
///
/// `fs::copy` izin bitlerini kopyalar ama mtime'ı kopyalamaz. Başarısızlık
/// **yedeği düşürmez**: içeriği doğru ama zaman damgası yanlış bir yedek, hiç
/// yedek olmamasından çok daha iyidir. Windows'ta salt-okunur bir kaynağın
/// kopyası da salt-okunur olur ve mtime yazımı reddedilir; o durumda bayrak
/// geçici olarak kaldırılıp geri konur.
///
/// Zaman damgası parametre olarak geliyor, kaynaktan yeniden okunmuyor: gezinme
/// sırasında `walkdir` o `stat` çağrısını zaten yapmış durumda.
fn apply_mtime(source: &Path, target: &Path, mtime: filetime::FileTime) {
    if filetime::set_file_mtime(target, mtime).is_ok() {
        return;
    }
    if retry_without_readonly(target, mtime) {
        return;
    }

    log::warn!("Değişiklik zamanı taşınamadı: {source:?} -> {target:?}");
}

/// Salt-okunur bayrağını geçici olarak kaldırıp mtime'ı yeniden dener.
///
/// Yalnızca Windows'ta derlenir. Unix'te `set_readonly(false)` dosyayı **herkese
/// yazılabilir** yapar (clippy `permissions_set_readonly_false` bunu haklı olarak
/// uyarıyor) ve zaten gereksizdir: orada mtime yazmak için dosyanın yazma izni
/// değil, sahipliği aranır.
#[cfg(windows)]
fn retry_without_readonly(target: &Path, mtime: filetime::FileTime) -> bool {
    let Ok(metadata) = fs::metadata(target) else {
        return false;
    };
    let mut permissions = metadata.permissions();
    if !permissions.readonly() {
        return false;
    }

    permissions.set_readonly(false);
    if fs::set_permissions(target, permissions).is_err() {
        return false;
    }

    let succeeded = filetime::set_file_mtime(target, mtime).is_ok();

    // Bayrağı geri koy: kopya kaynağıyla aynı izinlere sahip kalmalı.
    if let Ok(metadata) = fs::metadata(target) {
        let mut permissions = metadata.permissions();
        permissions.set_readonly(true);
        let _ = fs::set_permissions(target, permissions);
    }
    succeeded
}

#[cfg(not(windows))]
fn retry_without_readonly(_target: &Path, _mtime: filetime::FileTime) -> bool {
    false
}

/// Bir save dizinini kopyalar.
///
/// İptalde veya hatada yarım kalan hedef silinir. Geride bırakmak, save
/// klasörüne yarım kopyalanmış bir dizin koymak demektir ve uygulama (ile oyun)
/// onu gerçek bir save gibi listeler.
/// İptal kontrolü ile bayt bütçesinin dinlendiği parça boyutu.
///
/// `io::copy` bir akışı sonuna kadar kesintisiz kopyalar; yüzlerce megabaytlık
/// tek bir bölge dosyasında "İptal" o dosya bitene kadar yanıt vermiyordu.
/// 1 MiB, verimi düşürmeyecek kadar büyük, iptal gecikmesini hissettirmeyecek
/// kadar küçük.
const CANCEL_CHUNK: u64 = 1024 * 1024;

/// Parça parça kopyalar; her parçada iptali dinler ve `limit`'ten bir fazlasından
/// çoğunu yazmaz.
///
/// `limit` sınır yoksa `u64::MAX` verilir. Dönen değer yazılan bayt sayısı;
/// `limit`'i aşıp aşmadığına çağıran karar verir — böylece bu yardımcı hem
/// bütçesiz (dışa aktarma) hem bütçeli (içe aktarma) kullanılabiliyor.
fn copy_chunked(
    reader: &mut impl io::Read,
    writer: &mut impl io::Write,
    sink: &dyn ProgressSink,
    progress: (u64, u64),
    limit: u64,
    error_path: &Path,
) -> Result<u64, OpError> {
    // Bir fazlası: taşmanın olup olmadığı ancak sınırın ötesini okumaya
    // çalışarak anlaşılır.
    let ceiling = limit.saturating_add(1);
    let mut written = 0u64;

    while written < ceiling {
        let want = CANCEL_CHUNK.min(ceiling - written);
        let copied = io::copy(&mut io::Read::take(&mut *reader, want), writer)
            .map_err(OpError::io(error_path))?;
        written += copied;
        if copied < want {
            break; // kaynak bitti
        }
        sink.tick(progress.0, progress.1)?;
    }

    Ok(written)
}

/// Kopyalanacak bir dosya: kaynağa göre yolu ve gezinmede okunan mtime'ı.
///
/// Yalnızca **göreli** yol saklanıyor; mutlak yol `source.join(...)` ile
/// kuruluyor. Girdi başına iki `PathBuf` tutmak, on binlerce dosyalı bir save'de
/// gereksiz onlarca MB demekti.
struct PendingCopy {
    relative: PathBuf,
    mtime: Option<filetime::FileTime>,
}

pub fn copy_save(
    source: &Path,
    destination: &Path,
    sink: &dyn ProgressSink,
) -> Result<(), OpError> {
    let guard = CleanupGuard::arm(destination, PathKind::Dir);
    fs::create_dir_all(destination).map_err(OpError::io(destination))?;

    // **Tek gezinme.** Eskiden ağaç iki kez geziliyordu (bir dosyalar, bir de boş
    // dizinler korunsun diye dizinler) ve her dosya için ayrıca
    // `create_dir_all(parent)` ile `fs::metadata` çağrılıyordu. Ölçüm (20.000
    // dosya, NVMe): 991 ms → 877 ms; `create_dir_all` 20.400 → 401, gereksiz
    // `metadata` 20.000 → 0.
    //
    // Dizinler görüldükleri anda oluşturuluyor. `walkdir` üstten alta ilerlediği
    // için üst dizin zaten önce geliyor, ama yine de `create_dir_all`
    // kullanılıyor: sıraya yaslanmayan kod, sıralama değişirse sessizce bozulmaz.
    let mut files: Vec<PendingCopy> = Vec::new();
    for entry in WalkDir::new(source).sort_by_file_name() {
        let entry = entry.map_err(walk_error(source))?;
        if entry.depth() == 0 {
            continue;
        }
        let relative = entry
            .path()
            .strip_prefix(source)
            .expect("walkdir kökün altındaki yolları üretir")
            .to_path_buf();

        if entry.file_type().is_dir() {
            let target = destination.join(&relative);
            fs::create_dir_all(&target).map_err(OpError::io(&target))?;
        } else {
            files.push(PendingCopy {
                mtime: entry
                    .metadata()
                    .ok()
                    .map(|meta| filetime::FileTime::from_last_modification_time(&meta)),
                relative,
            });
        }
    }

    let total = files.len() as u64;
    for (index, file) in files.iter().enumerate() {
        sink.tick(index as u64 + 1, total)?;

        let from = source.join(&file.relative);
        let target = destination.join(&file.relative);
        fs::copy(&from, &target).map_err(OpError::io(&target))?;
        if let Some(mtime) = file.mtime {
            apply_mtime(&from, &target, mtime);
        }
    }

    guard.disarm();
    Ok(())
}

/// Geri yükleme yarıda kalırsa eski save'i yerine koyan nöbetçi.
struct RestoreGuard {
    save: PathBuf,
    safety: PathBuf,
    armed: bool,
}

impl RestoreGuard {
    fn disarm(mut self) {
        self.armed = false;
    }
}

impl Drop for RestoreGuard {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        // Yarım kalan kopya silinir, sonra eski save adına geri döner. İkisi de
        // başarısız olabilir; o durumda kullanıcının verisi hâlâ `safety`
        // adındaki klasörde duruyor — kayıp yok, yalnızca adı değişik.
        let _ = fs::remove_dir_all(&self.save);
        let _ = fs::rename(&self.safety, &self.save);
    }
}

/// Bir yedeği save'in üzerine geri yükler.
///
/// **Yıkıcı değil.** Mevcut save silinmez; `safety` adına **yeniden adlandırılır**
/// ve böylece geçmişte yeni bir yedek olarak görünür. Kopyalamak yerine yeniden
/// adlandırmak anlıktır, diski iki katına çıkarmaz ve başarısızlıkta geri
/// alınabilir — üç özellik de bir yedekleme aracında kopyalamaya tercih edilir.
/// `safety` aynı map klasöründe olduğu için yeniden adlandırma hep aynı dosya
/// sisteminde kalır.
///
/// Yanlış yedeği geri yükleyen kullanıcı, işlemden önceki hâle geçmişteki yeni
/// kayıttan dönebilir.
pub fn restore_save(
    backup: &Path,
    save: &Path,
    safety: &Path,
    sink: &dyn ProgressSink,
) -> Result<(), OpError> {
    fs::rename(save, safety).map_err(OpError::io(save))?;

    let guard = RestoreGuard {
        save: save.to_path_buf(),
        safety: safety.to_path_buf(),
        armed: true,
    };
    copy_save(backup, save, sink)?;
    guard.disarm();
    Ok(())
}

/// Bir save dizinini siler.
///
/// Bilerek iptal edilebilir değil: yarıda durmak kısmen silinmiş bir save
/// bırakır ve bu, işi bitirmekten daha kötüdür.
pub fn delete_save(source: &Path, sink: &dyn ProgressSink) -> Result<(), OpError> {
    fs::remove_dir_all(source).map_err(OpError::io(source))?;
    // İptal edilemeyen bir işlemde `tick`'in iptal sinyali anlamsız; yalnızca
    // ilerleme bildirmek için çağrılıyor.
    let _ = sink.tick(1, 1);
    Ok(())
}

/// Zip girdi adı: her platformda `/` ile ayrılmalı.
///
/// Windows'ta `Path` bileşenleri `\` ile birleşir ve öyle yazılan bir arşiv
/// Linux/macOS'ta tek bir uzun dosya adı gibi görünür. Python'un
/// `zipfile.write`'ı bu dönüşümü kendiliğinden yapıyordu.
fn zip_entry_name(relative: &Path) -> String {
    relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

/// Bir save dizinini zip'ler.
pub fn export_save(
    source: &Path,
    zip_path: &Path,
    compression_level: i64,
    sink: &dyn ProgressSink,
) -> Result<(), OpError> {
    let files = walk_files(source)?;
    let total = files.len() as u64;
    // Arşivin kökünde save adının kalması için bir üst dizinden göreceleştiriyoruz.
    let base = source.parent().unwrap_or(source);

    let guard = CleanupGuard::arm(zip_path, PathKind::File);
    let file = fs::File::create(zip_path).map_err(OpError::io(zip_path))?;
    let mut writer = zip::ZipWriter::new(io::BufWriter::new(file));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .compression_level(Some(compression_level));

    for (index, path) in files.iter().enumerate() {
        sink.tick(index as u64 + 1, total)?;

        let relative = path
            .strip_prefix(base)
            .expect("walkdir kökün altındaki yolları üretir");
        writer.start_file(zip_entry_name(relative), options)?;
        let mut input = fs::File::open(path).map_err(OpError::io(path))?;
        // Akışla kopyalanır: büyük bir bölge dosyası belleğe alınmaz. Parçalı
        // olması iptalin dosya ortasında da yanıt vermesini sağlıyor.
        copy_chunked(
            &mut input,
            &mut writer,
            sink,
            (index as u64 + 1, total),
            u64::MAX,
            path,
        )?;
    }

    writer.finish()?;
    guard.disarm();
    Ok(())
}

/// Açık bir arşivdeki üst düzey adlardan `target_dir` içinde zaten var olanlar.
///
/// **Her** üst düzey girdi kontrol edilir. Yalnızca ilkine bakmak (Python'un ilk
/// hali böyleydi), çok köklü bir arşivin ya da ilk girdisi serbest bir dosya olan
/// arşivin var olan save'lerin üzerine uyarısız yazmasına izin veriyordu.
///
/// Yol değil **açık arşiv** alıyor: hem [`archive_conflicts`] hem [`import_save`]
/// aynı mantığı kullansın diye, ve daha önemlisi `import_save` kontrolü kendi
/// tuttuğu tanıtıcı üzerinden yapabilsin diye — bkz. oradaki açıklama.
fn conflicts_in<R: io::Read + io::Seek>(
    archive: &zip::ZipArchive<R>,
    target_dir: &Path,
) -> Vec<String> {
    let mut top_level = std::collections::BTreeSet::new();
    for name in archive.file_names() {
        if name.trim().is_empty() {
            continue;
        }
        if let Some(first) = name.replace('\\', "/").split('/').next()
            && !first.is_empty()
            && is_plain_component(first)
        {
            top_level.insert(first.to_string());
        }
    }

    top_level
        .into_iter()
        .filter(|name| target_dir.join(name).exists())
        .collect()
}

/// Bileşen sıradan bir ad mı — `..`, `.`, kök ya da sürücü harfi değil.
///
/// Çakışma taramasının bunu elemesi şart. `..` ile başlayan bir girdide
/// `target_dir.join("..")` **her zaman** var olduğu için, hedefin dışına çıkmaya
/// çalışan her arşiv yanıltıcı bir "çakışma" hatasına dönüşürdü. Kaçış denemesi
/// bir çakışma değil güvenlik hatasıdır; `import_save` içindeki `enclosed_name()`
/// onu kendi adıyla reddediyor.
fn is_plain_component(component: &str) -> bool {
    matches!(
        Path::new(component).components().next(),
        Some(std::path::Component::Normal(_))
    )
}

/// Arşivdeki üst düzey adlardan `target_dir` içinde zaten var olanlar.
///
/// Arayüzün ön kontrolü: ilerleme penceresi açılmadan, kullanıcıya çakışan
/// adları gösterebilmek için. Çıkarmanın güvenliği buna **dayanmaz**;
/// [`import_save`] aynı kontrolü kendisi de yapar.
pub fn archive_conflicts(zip_path: &Path, target_dir: &Path) -> Result<Vec<String>, OpError> {
    let file = fs::File::open(zip_path).map_err(OpError::io(zip_path))?;
    let archive = zip::ZipArchive::new(io::BufReader::new(file))?;
    Ok(conflicts_in(&archive, target_dir))
}

/// Yeni oluşturulan üst düzey girdileri, işlem yarıda kalırsa geri alan nöbetçi.
///
/// Bu **güvenli**, çünkü [`import_save`] çıkarmaya başlamadan önce çakışma
/// kontrolünü kendi açtığı arşiv üzerinde yapar: hedefte çakışan bir üst düzey ad
/// olmadığı garanti, dolayısıyla sildiğimiz her şey bu işlemin kendi ürettiği şeydir.
///
/// Kontrol bilerek `import_save`'in içinde. Önceden yalnızca çağıranın
/// [`archive_conflicts`]'i çağırmış olmasına güveniliyordu; kontrolü atlayan bir
/// çağıran, bir hata anında bu nöbetçinin kullanıcının **var olan** save'lerini
/// silmesine yol açardı. Ön koşul artık yorumla değil yapıyla korunuyor —
/// [`ProgressSink`]'in ilerleme ile iptali birleştirmesiyle aynı gerekçe.
struct RollbackGuard {
    target_dir: PathBuf,
    roots: std::collections::BTreeSet<String>,
    armed: bool,
}

impl RollbackGuard {
    fn new(target_dir: &Path) -> Self {
        Self {
            target_dir: target_dir.to_path_buf(),
            roots: std::collections::BTreeSet::new(),
            armed: true,
        }
    }

    fn record(&mut self, relative: &Path) {
        if let Some(first) = relative.components().next() {
            self.roots
                .insert(first.as_os_str().to_string_lossy().into_owned());
        }
    }

    fn disarm(mut self) {
        self.armed = false;
    }
}

impl Drop for RollbackGuard {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        for root in &self.roots {
            let path = self.target_dir.join(root);
            let _ = if path.is_dir() {
                fs::remove_dir_all(&path)
            } else {
                fs::remove_file(&path)
            };
        }
    }
}

/// Bir arşivi doğruladıktan sonra `target_dir` içine açar.
///
/// **Python'dan bilinçli sapma.** Orada CPython'un `ZipFile._extract_member`
/// fonksiyonunun sürücü harflerini ve `..` bileşenlerini sessizce kırpmasına
/// güveniliyordu; kaçmaya çalışan bir girdi sessizce hedefin içine düşüyordu.
/// `enclosed_name()` böyle bir girdide `None` döndürür ve biz arşivi
/// reddediyoruz: bozuk ya da kötü niyetli bir arşivin kullanıcıdan gizlenmesi
/// için bir neden yok.
pub fn import_save(
    zip_path: &Path,
    target_dir: &Path,
    max_bytes: u64,
    sink: &dyn ProgressSink,
) -> Result<(), OpError> {
    let file = fs::File::open(zip_path).map_err(OpError::io(zip_path))?;
    let mut archive = zip::ZipArchive::new(io::BufReader::new(file))?;

    if archive.is_empty() {
        return Err(OpError::EmptyArchive);
    }

    // Çakışma kontrolü **bu fonksiyonun kendi tuttuğu** tanıtıcı üzerinden.
    // Arayüz de ayrıca önden kontrol ediyor (ilerleme penceresi açılmadan hata
    // gösterebilmek için), ama `RollbackGuard`'ın güvenliği artık o çağrının
    // yapılmış olmasına dayanmıyor. Yan fayda: kontrol ile çıkarma arasında
    // arşiv dosyası değiştirilemez — aradaki tanıtıcı açık kalıyor, oysa iki ayrı
    // açılış arasında dosya takas edilebilirdi.
    let conflicts = conflicts_in(&archive, target_dir);
    if !conflicts.is_empty() {
        return Err(OpError::Conflicts(conflicts));
    }

    // Ucuz eleme: arşivin **kendi bildirdiği** boyut. Asıl sınır aşağıda, gerçekten
    // yazılan bayt üzerinden uygulanıyor — merkezi dizindeki boyut alanını arşivi
    // üreten yazar ve yalan söyleyebilir.
    //
    // `decompressed_size` toplamı bellekteki merkezi dizinden okur; hiç G/Ç
    // yapmaz. Eskiden bu döngü her girdi için `by_index` çağırıyor, o da girdinin
    // **yerel başlığına konumlanıp okuyordu** — 3.000 girdili bir arşivde 3.000
    // fazladan `seek`+`read`, ölçülen ~5,4 ms (soğuk önbellekte çok daha fazla).
    // Veri tanımlayıcısı kullanan bir girdi varsa `None` döner; o durumda eleme
    // atlanır ve iş tümüyle aşağıdaki bütçeye kalır — güvenli, çünkü asıl sınır
    // zaten orada.
    if let Some(declared) = archive.decompressed_size()
        && declared > u128::from(max_bytes)
    {
        return Err(OpError::TooLarge {
            actual: u64::try_from(declared).unwrap_or(u64::MAX),
            limit: max_bytes,
        });
    }

    let total = archive.len() as u64;
    let mut guard = RollbackGuard::new(target_dir);
    let mut budget = max_bytes;

    for index in 0..archive.len() {
        sink.tick(index as u64 + 1, total)?;

        let mut entry = archive.by_index(index)?;
        let relative = entry
            .enclosed_name()
            .ok_or_else(|| OpError::UnsafePath(entry.name().to_string()))?;
        guard.record(&relative);

        let output = target_dir.join(&relative);
        if entry.is_dir() {
            fs::create_dir_all(&output).map_err(OpError::io(&output))?;
            continue;
        }
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent).map_err(OpError::io(parent))?;
        }
        let mut target = fs::File::create(&output).map_err(OpError::io(&output))?;

        // Bütçe okumayı da durdurur, yani bomba diskte olduğu kadar işlemcide de
        // sınırlanmış olur: `take` sarmalayıcısı açılmayı sınırın ötesine
        // taşımıyor. Yarım kalan çıktıyı `RollbackGuard` topluyor.
        let written = copy_chunked(
            &mut entry,
            &mut target,
            sink,
            (index as u64 + 1, total),
            budget,
            &output,
        )?;
        if written > budget {
            return Err(OpError::ExtractionExceededLimit { limit: max_bytes });
        }
        budget -= written;
    }

    guard.disarm();
    Ok(())
}
