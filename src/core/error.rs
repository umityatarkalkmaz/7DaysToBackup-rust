use std::path::{Path, PathBuf};

/// Bir save işleminin başarısızlık nedeni.
///
/// `Cancelled` bilerek bir hata varyantı: Python'daki `OperationCancelled`
/// istisnasının karşılığı ve `?` ile çağrı yığınının tepesine kadar taşınıyor.
/// Çağıran tarafta temiz bir duruş olarak ele alınır, hata olarak değil.
#[derive(Debug, thiserror::Error)]
pub enum OpError {
    #[error("işlem iptal edildi")]
    Cancelled,

    /// Hangi yolun başarısız olduğunu taşır. Çıplak bir `io::Error` "dosya
    /// bulunamadı" der ama hangi dosya olduğunu söylemez; kullanıcıya bu haliyle
    /// gösterilecek bir mesaj için bu yeterli değil.
    #[error("{path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("arşiv okunamadı: {0}")]
    Zip(#[from] zip::result::ZipError),

    #[error("arşiv boş")]
    EmptyArchive,

    #[error("arşiv {} GB'a açılıyor, {} GB sınırının üzerinde", as_gb(*actual), as_gb(*limit))]
    TooLarge { actual: u64, limit: u64 },

    /// Arşivde hedef dizinin dışına yazmaya çalışan bir girdi var.
    #[error("arşivde güvenli olmayan yol: {0}")]
    UnsafePath(String),
}

impl OpError {
    /// `io::Error`'u yol bilgisiyle sarmalamak için kısa yol.
    ///
    /// Kullanımı: `fs::copy(a, b).map_err(OpError::io(&b))?`
    pub fn io(path: impl AsRef<Path>) -> impl FnOnce(std::io::Error) -> Self {
        let path = path.as_ref().to_path_buf();
        move |source| OpError::Io { path, source }
    }
}

fn as_gb(bytes: u64) -> String {
    format!("{:.1}", bytes as f64 / 1024.0_f64.powi(3))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn io_error_message_names_the_path() {
        let err = OpError::io("/tmp/save/player.ttp")(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "erişim reddedildi",
        ));
        let text = err.to_string();
        assert!(text.contains("/tmp/save/player.ttp"), "{text}");
        assert!(text.contains("erişim reddedildi"), "{text}");
    }

    #[test]
    fn too_large_reports_both_sizes_in_gb() {
        let err = OpError::TooLarge {
            actual: 25 * 1024_u64.pow(3),
            limit: 20 * 1024_u64.pow(3),
        };
        assert_eq!(
            err.to_string(),
            "arşiv 25.0 GB'a açılıyor, 20.0 GB sınırının üzerinde"
        );
    }
}
