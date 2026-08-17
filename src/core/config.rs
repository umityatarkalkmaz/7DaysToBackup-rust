//! Kullanıcı ayarlarının okunması ve atomik olarak yazılması.

use serde::{Deserialize, Serialize};
use std::io;
use std::path::Path;

/// Diske yazılan ayarlar.
///
/// Python sürümü serbest biçimli bir sözlük tutuyor ve anahtarları çağrı
/// yerlerinde dize olarak yazıyordu. Burada alanlar tiplenmiş: yanlış yazılmış
/// bir anahtar derleme hatası olur, sessizce `None` dönmez.
// `Eq` yok: `ui_scale` bir `f32` ve kayan nokta `Eq` uygulamaz. Testlerdeki
// `assert_eq!` karşılaştırmaları `PartialEq` ile çalışmaya devam ediyor.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Boş dize "otomatik algıla" demektir — Python ile aynı sözleşme.
    pub custom_save_path: String,
    /// Boş dize "varsayılan dili kullan" demektir.
    pub language: String,
    /// Arayüz ölçeği. `0.0` = otomatik algıla.
    ///
    /// egui, Qt'nin aksine masaüstünün yazı tipi boyutunu okumaz; ölçeği yalnızca
    /// pencere yöneticisinin bildirdiği katsayıdan alır
    /// (`pixels_per_point = zoom_factor * native_pixels_per_point`). Monitör
    /// ölçeği %100 iken bu katsayı 1.0'dır ve 1440p bir panelde varsayılan 13 px
    /// gövde metni, Python/Qt sürümüne göre belirgin biçimde küçük kalıyordu.
    /// `Ctrl` + `+` çalışıyor ama eframe'in `persistence` özelliği kapalı olduğu
    /// için her açılışta sıfırlanıyordu; bu alan onu kalıcı yapıyor.
    ///
    /// **Ham okunmaz** — [`Config::ui_scale`] üzerinden alınır.
    pub ui_scale: f32,
    /// Otomatik yedekleme aralığı, dakika. `0` = kapalı.
    ///
    /// **Ham okunmaz** — [`Config::auto_backup_interval`] üzerinden alınır.
    pub auto_backup_minutes: u32,
    /// Otomatik yedeklemenin save başına saklayacağı yedek sayısı. `0` = sınırsız.
    ///
    /// **Ham okunmaz** — [`Config::auto_backup_keep`] üzerinden alınır.
    pub auto_backup_keep: u32,
}

/// Kabul edilen en küçük ve en büyük arayüz ölçeği.
///
/// egui'nin kendi hata ayıklama arayüzü `0.10..=10.0` aralığını kullanıyor; bu
/// aralık daha dar, çünkü uçlarda arayüz kullanılamaz hâle geliyor.
const MIN_UI_SCALE: f32 = 0.5;
const MAX_UI_SCALE: f32 = 3.0;

/// Otomatik yedekleme aralığının sınırları, dakika.
///
/// Alt sınır 1: dakikadan sık yedek almak diski doldurur ve oyunu yavaşlatır.
/// Üst sınır bir gün: daha seyreği "kapalı" demenin dolambaçlı yolu.
const MIN_AUTO_BACKUP_MINUTES: u32 = 1;
const MAX_AUTO_BACKUP_MINUTES: u32 = 24 * 60;

/// Saklanabilecek en fazla yedek. Sınırsız için alan `0` bırakılır.
const MAX_AUTO_BACKUP_KEEP: u32 = 100;

impl Config {
    /// Doğrulanmış arayüz ölçeği; otomatik moddaysa `None`.
    ///
    /// `config.json` düz metin ve kullanıcı dizininde; bozulması zaten öngörülmüş
    /// bir senaryo (bkz. [`Config::load`]). `"ui_scale": 1e400` geçerli JSON'dur
    /// ve `f32::INFINITY` olarak ayrıştırılır; negatif ya da çok küçük değerler de
    /// geçerlidir. `egui::Context::set_zoom_factor` hiçbir doğrulama yapmıyor —
    /// ne kırpma ne `is_finite` — ve değer doğrudan font rasterleştirmeye gidiyor.
    /// Doğrulama tek yerde, burada.
    pub fn ui_scale(&self) -> Option<f32> {
        if !self.ui_scale.is_finite() || self.ui_scale <= 0.0 {
            return None;
        }
        Some(self.ui_scale.clamp(MIN_UI_SCALE, MAX_UI_SCALE))
    }

    /// Doğrulanmış otomatik yedekleme aralığı; kapalıysa `None`.
    ///
    /// `ui_scale` ile aynı gerekçe: `config.json` kullanıcı dizininde ve
    /// bozulması öngörülmüş bir senaryo. Sıfır kapalı demek, geri kalanı
    /// kullanışlı bir aralığa kırpılıyor.
    pub fn auto_backup_interval(&self) -> Option<std::time::Duration> {
        if self.auto_backup_minutes == 0 {
            return None;
        }
        let minutes = self
            .auto_backup_minutes
            .clamp(MIN_AUTO_BACKUP_MINUTES, MAX_AUTO_BACKUP_MINUTES);
        Some(std::time::Duration::from_secs(u64::from(minutes) * 60))
    }

    /// Save başına saklanacak yedek sayısı; sınırsızsa `None`.
    pub fn auto_backup_keep(&self) -> Option<usize> {
        if self.auto_backup_keep == 0 {
            return None;
        }
        Some(self.auto_backup_keep.min(MAX_AUTO_BACKUP_KEEP) as usize)
    }
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
                log::warn!("Ayar dosyası okunamadı ({path:?}): {err}");
                return Self::default();
            }
        };

        serde_json::from_str(&text).unwrap_or_else(|err| {
            log::warn!("Ayar dosyası bozuk ({path:?}): {err}");
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
            ..Default::default()
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
    fn ui_scale_defaults_to_automatic() {
        assert_eq!(Config::default().ui_scale(), None);
    }

    #[test]
    fn a_sane_ui_scale_passes_through() {
        let config = Config {
            ui_scale: 1.25,
            ..Default::default()
        };
        assert_eq!(config.ui_scale(), Some(1.25));
    }

    #[test]
    fn a_hostile_ui_scale_cannot_reach_the_renderer() {
        // `set_zoom_factor` hiçbir doğrulama yapmıyor ve değer doğrudan font
        // rasterleştirmeye gidiyor; sonsuz ya da negatif bir ölçek uygulamayı
        // arayüz çizilmeden öldürebilirdi.
        for hostile in [f32::INFINITY, f32::NEG_INFINITY, f32::NAN, -1.0, 0.0] {
            let config = Config {
                ui_scale: hostile,
                ..Default::default()
            };
            match config.ui_scale() {
                None => {}
                Some(value) => panic!("{hostile} otomatik moda düşmeliydi, gelen: {value}"),
            }
        }

        // Sonlu ama uçlarda olan değerler kırpılır, reddedilmez.
        for (raw, expected) in [(1e30_f32, MAX_UI_SCALE), (0.0001, MIN_UI_SCALE)] {
            let config = Config {
                ui_scale: raw,
                ..Default::default()
            };
            assert_eq!(config.ui_scale(), Some(expected), "ham değer {raw}");
        }
    }

    #[test]
    fn a_corrupt_ui_scale_on_disk_does_not_panic() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        // 1e400 geçerli JSON; f64 aralığını aştığı için sonsuza ayrıştırılır.
        std::fs::write(&path, r#"{"language":"tr","ui_scale":1e400}"#).unwrap();

        let config = Config::load(&path);
        assert_eq!(config.ui_scale(), None);
    }

    #[test]
    fn auto_backup_is_off_by_default() {
        let config = Config::default();
        assert_eq!(config.auto_backup_interval(), None);
        assert_eq!(config.auto_backup_keep(), None);
    }

    #[test]
    fn an_absurd_auto_backup_interval_is_clamped() {
        // Saniyede bir yedek diski doldurur; on yılda bir "kapalı" demenin
        // dolambaçlı yoludur. İkisi de kullanışlı aralığa çekiliyor.
        let fast = Config {
            auto_backup_minutes: 1,
            ..Default::default()
        };
        assert_eq!(
            fast.auto_backup_interval(),
            Some(std::time::Duration::from_secs(60))
        );

        let slow = Config {
            auto_backup_minutes: u32::MAX,
            ..Default::default()
        };
        assert_eq!(
            slow.auto_backup_interval(),
            Some(std::time::Duration::from_secs(
                u64::from(MAX_AUTO_BACKUP_MINUTES) * 60
            ))
        );
    }

    #[test]
    fn the_retention_count_is_clamped_too() {
        let config = Config {
            auto_backup_keep: u32::MAX,
            ..Default::default()
        };
        assert_eq!(
            config.auto_backup_keep(),
            Some(MAX_AUTO_BACKUP_KEEP as usize)
        );
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
            ..Default::default()
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
