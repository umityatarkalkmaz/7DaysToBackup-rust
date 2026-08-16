//! Kullanıcıya özel dizin çözümlemesi.
//!
//! Bilerek bağımlılıksız: `config`, `log` ve `platform` bu modülü kullanır, bu
//! yüzden buradan onlara doğru bir import döngü yaratır.

use std::path::PathBuf;

/// Python sürümü `7DaysToBackup` kullanıyor. Rust sürümü kasıtlı olarak ayrı bir
/// isim alıyor ki iki uygulama yan yana kurulduğunda birbirinin ayarını ezmesin.
pub const APP_NAME: &str = "7DaysToBackup-rust";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    Windows,
    MacOs,
    Linux,
}

impl Os {
    /// Derleme hedefinden okunur. `cfg!` bu dosyada tek bir yerde geçsin diye
    /// ayrı bir fonksiyon: geri kalan her şey `Os`'u parametre olarak alır ve
    /// böylece üç dal da her platformda test edilebilir.
    pub fn current() -> Self {
        if cfg!(target_os = "windows") {
            Self::Windows
        } else if cfg!(target_os = "macos") {
            Self::MacOs
        } else {
            Self::Linux
        }
    }
}

/// Ortam değişkenleri ve ev dizini. Testlerin gerçek ortamı kirletmeden sahte
/// değer verebilmesi için trait.
pub trait Env {
    fn var(&self, key: &str) -> Option<String>;
    fn home(&self) -> PathBuf;
}

pub struct RealEnv;

impl Env for RealEnv {
    fn var(&self, key: &str) -> Option<String> {
        // Boş dize `None` sayılır: Python tarafı `os.environ.get(...) or _home()`
        // yazıyor ve orada boş dize zaten yedeğe düşüyor. Aynı davranış.
        std::env::var(key).ok().filter(|value| !value.is_empty())
    }

    fn home(&self) -> PathBuf {
        dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
    }
}

/// Kullanıcıya özel yapılandırma dizini.
///
/// Windows'ta Belgeler/Masaüstü yerine APPDATA kullanılıyor: APPDATA hiçbir zaman
/// OneDrive Known Folder Move'a takılmaz, bu yüzden ek bir çözümlemeye gerek kalmaz.
pub fn config_dir_for(os: Os, env: &dyn Env) -> PathBuf {
    let base = match os {
        Os::Windows => env
            .var("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| env.home()),
        Os::MacOs => env.home().join("Library").join("Application Support"),
        Os::Linux => env
            .var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| env.home().join(".config")),
    };
    base.join(APP_NAME)
}

/// Kullanıcıya özel günlük dizini.
pub fn log_dir_for(os: Os, env: &dyn Env) -> PathBuf {
    let base = match os {
        Os::Windows => env
            .var("LOCALAPPDATA")
            .or_else(|| env.var("APPDATA"))
            .map(PathBuf::from)
            .unwrap_or_else(|| env.home()),
        Os::MacOs => env.home().join("Library").join("Logs"),
        Os::Linux => env
            .var("XDG_STATE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| env.home().join(".local").join("state")),
    };
    base.join(APP_NAME)
}

pub fn config_dir() -> PathBuf {
    config_dir_for(Os::current(), &RealEnv)
}

pub fn log_dir() -> PathBuf {
    log_dir_for(Os::current(), &RealEnv)
}

pub fn config_file() -> PathBuf {
    config_dir().join("config.json")
}

#[cfg(test)]
pub(crate) mod test_env {
    use super::{Env, PathBuf};
    use std::collections::HashMap;

    pub struct FakeEnv {
        pub vars: HashMap<String, String>,
        pub home: PathBuf,
    }

    impl FakeEnv {
        pub fn new(home: &str) -> Self {
            Self {
                vars: HashMap::new(),
                home: PathBuf::from(home),
            }
        }

        pub fn with(mut self, key: &str, value: &str) -> Self {
            self.vars.insert(key.to_string(), value.to_string());
            self
        }
    }

    impl Env for FakeEnv {
        fn var(&self, key: &str) -> Option<String> {
            self.vars.get(key).filter(|v| !v.is_empty()).cloned()
        }

        fn home(&self) -> PathBuf {
            self.home.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_env::FakeEnv;
    use super::*;

    #[test]
    fn config_and_log_dirs_are_app_scoped() {
        let env = FakeEnv::new("/home/u");
        for os in [Os::Windows, Os::MacOs, Os::Linux] {
            assert!(config_dir_for(os, &env).ends_with(APP_NAME));
            assert!(log_dir_for(os, &env).ends_with(APP_NAME));
        }
    }

    #[test]
    fn config_dir_uses_appdata_on_windows() {
        let env = FakeEnv::new(r"C:\Users\u").with("APPDATA", r"C:\Users\u\AppData\Roaming");
        assert_eq!(
            config_dir_for(Os::Windows, &env),
            PathBuf::from(r"C:\Users\u\AppData\Roaming").join(APP_NAME)
        );
    }

    #[test]
    fn config_dir_falls_back_to_home_when_appdata_is_unset() {
        let env = FakeEnv::new(r"C:\Users\u");
        assert_eq!(
            config_dir_for(Os::Windows, &env),
            PathBuf::from(r"C:\Users\u").join(APP_NAME)
        );
    }

    #[test]
    fn config_dir_honours_xdg_on_linux() {
        let env = FakeEnv::new("/home/u").with("XDG_CONFIG_HOME", "/xdg");
        assert_eq!(
            config_dir_for(Os::Linux, &env),
            PathBuf::from("/xdg").join(APP_NAME)
        );
    }

    #[test]
    fn config_dir_defaults_to_dot_config_on_linux() {
        let env = FakeEnv::new("/home/u");
        assert_eq!(
            config_dir_for(Os::Linux, &env),
            PathBuf::from("/home/u/.config").join(APP_NAME)
        );
    }

    #[test]
    fn config_dir_uses_application_support_on_macos() {
        let env = FakeEnv::new("/Users/u");
        assert_eq!(
            config_dir_for(Os::MacOs, &env),
            PathBuf::from("/Users/u/Library/Application Support").join(APP_NAME)
        );
    }

    #[test]
    fn empty_env_var_is_treated_as_unset() {
        // Python `os.environ.get("APPDATA") or _home()` yazıyor; boş dize oradaki
        // `or` ile yedeğe düşer. Aynı davranışı koruyoruz.
        let env = FakeEnv::new(r"C:\Users\u").with("APPDATA", "");
        assert_eq!(
            config_dir_for(Os::Windows, &env),
            PathBuf::from(r"C:\Users\u").join(APP_NAME)
        );
    }

    #[test]
    fn log_dir_prefers_localappdata_then_appdata_on_windows() {
        let base = FakeEnv::new(r"C:\Users\u");
        let both = FakeEnv::new(r"C:\Users\u")
            .with("LOCALAPPDATA", r"C:\Local")
            .with("APPDATA", r"C:\Roaming");
        assert_eq!(
            log_dir_for(Os::Windows, &both),
            PathBuf::from(r"C:\Local").join(APP_NAME)
        );

        let roaming_only = FakeEnv::new(r"C:\Users\u").with("APPDATA", r"C:\Roaming");
        assert_eq!(
            log_dir_for(Os::Windows, &roaming_only),
            PathBuf::from(r"C:\Roaming").join(APP_NAME)
        );

        assert_eq!(
            log_dir_for(Os::Windows, &base),
            PathBuf::from(r"C:\Users\u").join(APP_NAME)
        );
    }

    #[test]
    fn log_dir_honours_xdg_state_on_linux() {
        let env = FakeEnv::new("/home/u").with("XDG_STATE_HOME", "/state");
        assert_eq!(
            log_dir_for(Os::Linux, &env),
            PathBuf::from("/state").join(APP_NAME)
        );

        let plain = FakeEnv::new("/home/u");
        assert_eq!(
            log_dir_for(Os::Linux, &plain),
            PathBuf::from("/home/u/.local/state").join(APP_NAME)
        );
    }

    #[test]
    fn log_dir_uses_library_logs_on_macos() {
        let env = FakeEnv::new("/Users/u");
        assert_eq!(
            log_dir_for(Os::MacOs, &env),
            PathBuf::from("/Users/u/Library/Logs").join(APP_NAME)
        );
    }

    #[test]
    fn app_name_is_distinct_from_the_python_build() {
        // İki sürümün aynı config.json'ı paylaşmaması bilinçli bir karar.
        assert_ne!(APP_NAME, "7DaysToBackup");
    }

    #[test]
    fn real_env_reports_a_home_directory() {
        assert!(!RealEnv.home().as_os_str().is_empty());
    }
}
