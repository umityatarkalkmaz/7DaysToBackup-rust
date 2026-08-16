//! Günlükleme kurulumu.
//!
//! Python sürümündeki kuralı koruyoruz: bu modül **hiçbir koşulda** uygulamayı
//! düşürmez. Salt-okunur bir konuma kurulum (ör. `C:\Program Files`) günlük
//! dosyası açamaz; bu, ortada henüz bir arayüz yokken uygulamayı öldürmek için
//! bir gerekçe değil.

use flexi_logger::{Cleanup, Criterion, Duplicate, FileSpec, LoggerHandle, Naming};
use std::path::Path;

/// Python `RotatingFileHandler(maxBytes=1_000_000, backupCount=3)` kullanıyor.
const MAX_LOG_BYTES: u64 = 1_000_000;
const KEPT_LOG_FILES: usize = 3;

/// `SEVENDAYS_DEBUG` tanımlıysa ayrıntılı günlük — Python ile aynı değişken adı.
///
/// Seviye bilerek kendi crate'imize daraltılıyor. Düz `"info"` yazıldığında
/// zbus, wgpu ve winit kendi INFO kayıtlarını da dosyaya döküyor ve tek bir
/// açılış 2 KB'lık gürültü üretiyor — kullanıcının hata bildirimiyle
/// gönderdiği günlük okunamaz hale geliyor. Python tarafında bu sorun yoktu,
/// çünkü orada yalnızca `7DaysToBackup` adlı logger dosyaya yazıyordu.
///
/// Üçüncü taraf kayıtları gerçekten gerekirse `RUST_LOG` bu ayarı geçersiz kılar
/// (`try_with_env_or_str`).
fn level_spec(debug_requested: bool) -> &'static str {
    if debug_requested {
        "warn, seven_days_to_backup=debug"
    } else {
        "warn, seven_days_to_backup=info"
    }
}

fn debug_requested() -> bool {
    std::env::var_os("SEVENDAYS_DEBUG").is_some_and(|value| !value.is_empty())
}

fn file_logger(
    level: &str,
    directory: &Path,
) -> Result<LoggerHandle, flexi_logger::FlexiLoggerError> {
    flexi_logger::Logger::try_with_env_or_str(level)?
        .log_to_file(
            FileSpec::default()
                .directory(directory)
                .basename("debug")
                .suffix("log"),
        )
        .rotate(
            Criterion::Size(MAX_LOG_BYTES),
            Naming::Numbers,
            Cleanup::KeepLogFiles(KEPT_LOG_FILES),
        )
        .duplicate_to_stderr(Duplicate::Info)
        .start()
}

/// Günlüklemeyi başlatır.
///
/// Dönen tutamaç **canlı tutulmalı**: düştüğünde flexi_logger arabelleği boşaltır
/// ve dosyayı kapatır. `main` onu uygulamanın ömrü boyunca elde tutar.
///
/// Dosyaya yazma kurulamazsa yalnızca stderr'e düşülür; o da olmazsa uygulama
/// günlüksüz devam eder.
pub fn init() -> Option<LoggerHandle> {
    let level = level_spec(debug_requested());
    let directory = crate::core::paths::log_dir();

    match file_logger(level, &directory) {
        Ok(handle) => Some(handle),
        Err(err) => {
            eprintln!(
                "Günlük dosyası açılamadı ({}): {err}. Yalnızca stderr kullanılacak.",
                directory.display()
            );
            flexi_logger::Logger::try_with_env_or_str(level)
                .and_then(flexi_logger::Logger::start)
                .ok()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_flag_selects_the_verbose_level() {
        assert!(level_spec(true).contains("seven_days_to_backup=debug"));
        assert!(level_spec(false).contains("seven_days_to_backup=info"));
    }

    #[test]
    fn third_party_crates_are_held_at_warn() {
        // Bu daraltma olmadan zbus/wgpu/winit tek bir açılışta günlüğü doldurur.
        for spec in [level_spec(true), level_spec(false)] {
            assert!(spec.starts_with("warn"), "{spec}");
        }
    }

    /// Süreç başına yalnızca bir global logger kaydedilebildiği için günlükleme
    /// tarafında logger başlatan **tek** test bu. İkincisi eklenirse
    /// `start()` `SetLoggerError` ile döner ve testler sıraya bağlı hale gelir.
    #[test]
    fn file_logger_writes_into_the_requested_directory() {
        let dir = tempfile::tempdir().unwrap();
        let handle = file_logger("info", dir.path()).expect("logger kurulmalı");

        // Dosya tembel oluşturuluyor: kayıt yazılana ve arabellek boşaltılana
        // kadar dizin boş kalır.
        ::log::info!("test kaydı");
        handle.flush();

        let created: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert!(
            created.iter().any(|name| name.contains("debug")),
            "günlük dosyası oluşmadı: {created:?}"
        );
    }
}
