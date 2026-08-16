//! 7 Days to Die save klasörünün ve masaüstünün bulunması.

use crate::core::config::Config;
use crate::core::paths::{Env, Os, RealEnv};
use std::path::{Path, PathBuf};

const GAME_DIR: &str = "7DaysToDie";
const SAVES_DIR: &str = "Saves";

/// Oyunun save klasörünün işletim sistemine göre varsayılan konumu.
pub fn default_saves_path_for(os: Os, env: &dyn Env) -> PathBuf {
    let base = match os {
        // Python `os.path.expandvars(r"%APPDATA%\...")` kullanıyor; APPDATA
        // tanımsızsa expandvars dizeyi olduğu gibi bırakır ve ortaya
        // "%APPDATA%\7DaysToDie\Saves" gibi kullanılamaz bir yol çıkar.
        // Ev dizinine düşmek en azından var olan bir yol üretir.
        Os::Windows => env
            .var("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| env.home()),
        Os::MacOs => env.home().join("Library").join("Application Support"),
        Os::Linux => env.home().join(".local").join("share"),
    };
    base.join(GAME_DIR).join(SAVES_DIR)
}

pub fn default_saves_path() -> PathBuf {
    default_saves_path_for(Os::current(), &RealEnv)
}

/// Kullanıcının tanımladığı yol geçerliyse onu, değilse varsayılanı döndürür.
///
/// Python'da bu fonksiyon global `config` tekilini okuyordu. Burada `&Config`
/// parametre: bağımlılık görünür, testler tekili sıfırlamak zorunda kalmaz ve
/// değerin import anında donması (Python'un `SAVES_PATH` regresyonu) mümkün değil.
pub fn saves_path(config: &Config) -> PathBuf {
    let custom = config.custom_save_path.trim();
    if !custom.is_empty() && Path::new(custom).is_dir() {
        return PathBuf::from(custom);
    }
    default_saves_path()
}

/// Dışa aktarmanın varsayılan hedefi olan masaüstü dizini.
///
/// Python burada Qt'nin `QStandardPaths`'ini çağırıyordu, çünkü sabit bir
/// `~/Desktop`, OneDrive Known Folder Move ile yönlendirilmiş veya İngilizce
/// olmayan Windows'ta yerelleştirilmiş masaüstünde yanlış sonuç veriyor.
/// Qt yok; `dirs::desktop_dir()` Windows'ta aynı `SHGetKnownFolderPath` API'sini
/// kullandığı için doğrudan karşılığı.
pub fn desktop_path() -> PathBuf {
    if let Some(desktop) = dirs::desktop_dir().filter(|path| path.is_dir()) {
        return desktop;
    }
    if Os::current() == Os::Linux
        && let Some(desktop) = RealEnv
            .var("XDG_DESKTOP_DIR")
            .map(PathBuf::from)
            .filter(|path| path.is_dir())
    {
        return desktop;
    }
    RealEnv.home().join("Desktop")
}

/// Dışa aktarmanın yazılacağı dizin: masaüstü yoksa ev dizini.
pub fn export_dir() -> PathBuf {
    let desktop = desktop_path();
    if desktop.is_dir() {
        return desktop;
    }
    RealEnv.home()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::paths::test_env::FakeEnv;

    #[test]
    fn default_saves_path_per_os() {
        let windows = FakeEnv::new(r"C:\Users\u").with("APPDATA", r"C:\Roaming");
        assert_eq!(
            default_saves_path_for(Os::Windows, &windows),
            PathBuf::from(r"C:\Roaming").join(GAME_DIR).join(SAVES_DIR)
        );

        let mac = FakeEnv::new("/Users/u");
        assert_eq!(
            default_saves_path_for(Os::MacOs, &mac),
            PathBuf::from("/Users/u/Library/Application Support/7DaysToDie/Saves")
        );

        let linux = FakeEnv::new("/home/u");
        assert_eq!(
            default_saves_path_for(Os::Linux, &linux),
            PathBuf::from("/home/u/.local/share/7DaysToDie/Saves")
        );
    }

    #[test]
    fn windows_without_appdata_still_yields_a_usable_path() {
        let env = FakeEnv::new(r"C:\Users\u");
        let path = default_saves_path_for(Os::Windows, &env);
        assert!(!path.to_string_lossy().contains('%'), "{path:?}");
        assert!(path.ends_with(Path::new(GAME_DIR).join(SAVES_DIR)));
    }

    #[test]
    fn custom_save_path_wins_when_it_is_a_real_directory() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config {
            custom_save_path: dir.path().to_string_lossy().into_owned(),
            ..Config::default()
        };
        assert_eq!(saves_path(&config), dir.path());
    }

    #[test]
    fn custom_save_path_is_ignored_when_it_does_not_exist() {
        let config = Config {
            custom_save_path: "/definitely/not/here".to_string(),
            ..Config::default()
        };
        assert_eq!(saves_path(&config), default_saves_path());
    }

    #[test]
    fn custom_save_path_is_ignored_when_it_is_a_file() {
        // `is_dir` kontrolü olmasa bir dosya yolu save klasörü sanılırdı.
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("not-a-directory");
        std::fs::write(&file, b"x").unwrap();

        let config = Config {
            custom_save_path: file.to_string_lossy().into_owned(),
            ..Config::default()
        };
        assert_eq!(saves_path(&config), default_saves_path());
    }

    #[test]
    fn custom_save_path_is_ignored_when_blank() {
        for blank in ["", "   "] {
            let config = Config {
                custom_save_path: blank.to_string(),
                ..Config::default()
            };
            assert_eq!(saves_path(&config), default_saves_path());
        }
    }

    #[test]
    fn saves_path_tracks_config_changes_rather_than_caching() {
        // Python'daki regresyonun karşılığı: değer import anında donmamalı.
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();

        let mut config = Config {
            custom_save_path: first.path().to_string_lossy().into_owned(),
            ..Config::default()
        };
        assert_eq!(saves_path(&config), first.path());

        config.custom_save_path = second.path().to_string_lossy().into_owned();
        assert_eq!(saves_path(&config), second.path());
    }

    #[test]
    fn export_dir_is_an_existing_directory() {
        assert!(export_dir().is_dir());
    }
}
