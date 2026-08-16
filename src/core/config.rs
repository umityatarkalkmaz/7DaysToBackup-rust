//! Kullanıcı ayarlarının okunması ve atomik olarak yazılması.

use serde::{Deserialize, Serialize};
use std::io;
use std::path::Path;

/// Diske yazılan ayarlar.
///
/// Python sürümü serbest biçimli bir sözlük tutuyor ve anahtarları çağrı
/// yerlerinde dize olarak yazıyordu. Burada alanlar tiplenmiş: yanlış yazılmış
/// bir anahtar derleme hatası olur, sessizce `None` dönmez.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Boş dize "otomatik algıla" demektir — Python ile aynı sözleşme.
    pub custom_save_path: String,
    /// Boş dize "varsayılan dili kullan" demektir.
    pub language: String,
}

impl Config {
    /// Ayarları okur. Hiçbir koşulda başarısız olmaz; okunamayan dosya
    /// varsayılanlara düşer.
    ///
    /// Bozuk dosya sessizce yutulmaz, günlüğe yazılır: kullanıcı ayarlarının
    /// neden sıfırlandığını görebilmeli.
    pub fn load(path: &Path) -> Self {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Self::default(),
            Err(err) => {
                log::warn!("Ayar dosyası okunamadı ({}): {err}", path.display());
                return Self::default();
            }
        };

        serde_json::from_str(&text).unwrap_or_else(|err| {
            log::warn!("Ayar dosyası bozuk ({}): {err}", path.display());
            Self::default()
        })
    }

    /// Ayarları atomik olarak yazar.
    ///
    /// `bool` değil `Result` döner: Python sürümünde ayarlar penceresi dönüş
    /// değerini kontrol etmek zorundaydı ve etmeyi unutmak sessiz veri kaybı
    /// demekti. `#[must_use]` bunu derleyicinin işi haline getirir.
    ///
    /// Geçici dosyaya yaz → fsync → yeniden adlandır sırası kasıtlı: yarıda
    /// kesilen bir yazma artık ayrıştırılamaz bir config.json bırakamaz.
    pub fn save(&self, path: &Path) -> Result<(), io::Error> {
        let directory = path.parent().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("ayar yolunun üst dizini yok: {}", path.display()),
            )
        })?;
        std::fs::create_dir_all(directory)?;

        // Drop olduğunda kendini siler; başarısız bir yazma geride .tmp bırakmaz.
        let mut temp = tempfile::NamedTempFile::new_in(directory)?;
        serde_json::to_writer_pretty(&mut temp, self)?;
        temp.as_file().sync_all()?;
        temp.persist(path).map_err(|err| err.error)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Config {
        Config {
            custom_save_path: "/tmp/saves".to_string(),
            language: "en".to_string(),
        }
    }

    #[test]
    fn round_trips_through_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");

        sample().save(&path).unwrap();
        assert_eq!(Config::load(&path), sample());
    }

    #[test]
    fn missing_file_yields_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config::load(&dir.path().join("nothing-here.json"));
        assert_eq!(config, Config::default());
    }

    #[test]
    fn creates_the_missing_directory() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("deeper").join("config.json");
        assert!(!path.parent().unwrap().exists());

        sample().save(&path).unwrap();
        assert!(path.is_file());
    }

    #[test]
    fn corrupt_file_falls_back_to_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        std::fs::write(&path, r#"{"truncated": "#).unwrap();

        assert_eq!(Config::load(&path), Config::default());
    }

    #[test]
    fn unknown_keys_do_not_discard_known_ones() {
        // Python sözlüğü fazladan anahtarları taşıyordu. serde varsayılan olarak
        // bilinmeyen alanları yok sayar; asıl önemlisi bilinenlerin okunması.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        std::fs::write(&path, r#"{"language":"tr","from_the_future":42}"#).unwrap();

        assert_eq!(Config::load(&path).language, "tr");
    }

    #[test]
    fn missing_keys_fall_back_per_field() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        std::fs::write(&path, r#"{"language":"en"}"#).unwrap();

        let config = Config::load(&path);
        assert_eq!(config.language, "en");
        assert_eq!(config.custom_save_path, "");
    }

    #[test]
    fn leaves_no_temp_files_behind() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        sample().save(&path).unwrap();
        sample().save(&path).unwrap();

        let entries: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(entries, vec![std::ffi::OsString::from("config.json")]);
    }

    #[test]
    fn save_reports_failure_instead_of_pretending() {
        // Üst dizinin yerinde bir dosya var; create_dir_all başarısız olmalı.
        let dir = tempfile::tempdir().unwrap();
        let blocker = dir.path().join("blocker");
        std::fs::write(&blocker, b"not a directory").unwrap();

        assert!(sample().save(&blocker.join("config.json")).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn existing_config_survives_a_failed_write() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        sample().save(&path).unwrap();
        let original = std::fs::read_to_string(&path).unwrap();

        // Dizini salt-okunur yap: geçici dosya oluşturulamaz.
        let mut perms = std::fs::metadata(dir.path()).unwrap().permissions();
        perms.set_mode(0o500);
        std::fs::set_permissions(dir.path(), perms).unwrap();

        let outcome = Config {
            custom_save_path: "/should/not/land".to_string(),
            language: "de".to_string(),
        }
        .save(&path);

        // Silinebilmesi için izinleri geri ver (assert'lerden önce, panik olsa da
        // tempdir temizlenebilsin).
        let mut perms = std::fs::metadata(dir.path()).unwrap().permissions();
        perms.set_mode(0o700);
        std::fs::set_permissions(dir.path(), perms).unwrap();

        assert!(outcome.is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
    }
}
