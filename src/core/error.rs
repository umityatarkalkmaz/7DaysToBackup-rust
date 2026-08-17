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
    ///
    /// `{path:?}` bilerek `Display` değil: bu mesaj günlüğe de yazılıyor ve
    /// Linux'ta dosya adı satır sonu içerebilir. Kaçış tek yerde, kaynağında
    /// yapılıyor; böylece `OpError`'u loglayan her çağrı kendiliğinden güvenli.
    #[error("{path:?}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("arşiv okunamadı: {0}")]
    Zip(#[from] zip::result::ZipError),

    #[error("arşiv boş")]
    EmptyArchive,

    /// Arşivin **kendi bildirdiği** boyut sınırın üzerinde.
    #[error("arşiv {} GB'a açılıyor, {} GB sınırının üzerinde", as_gb(*actual), as_gb(*limit))]
    TooLarge { actual: u64, limit: u64 },

    /// Arşiv bildirdiğinden fazlasını açtı ve sınır çıkarma sırasında aşıldı.
    ///
    /// [`OpError::TooLarge`]'dan ayrı bir varyant, çünkü orada gerçek boyut
    /// biliniyor; burada işlem sınıra varır varmaz durdurulduğu için bilinmiyor.
    /// Tek bir varyantla ikisini anlatmak, mesajın yalan söylemesi demekti.
    #[error("arşiv bildirdiğinden büyük: {} GB sınırı çıkarma sırasında aşıldı", as_gb(*limit))]
    ExtractionExceededLimit { limit: u64 },

    /// Hedefte, arşivin üst düzey adlarıyla çakışan girdiler var.
    ///
    /// Adlar taşınıyor: arayüz hangi save'lerin çakıştığını gösterebilmeli.
    #[error("hedefte aynı adlı girdiler var: {}", .0.join(", "))]
    Conflicts(Vec<String>),

    /// Arşivde hedef dizinin dışına yazmaya çalışan bir girdi var.
    ///
    /// `{0:?}` bilerek: bu dize tümüyle arşivi üretenin denetiminde ve mesaj
    /// günlüğe de yazılıyor. `Display` ile yazılsaydı, adında satır sonu olan bir
    /// girdi günlük dosyasına sahte kayıt satırları ekleyebilirdi.
    #[error("arşivde güvenli olmayan yol: {0:?}")]
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
    fn a_path_with_a_newline_cannot_forge_a_log_line() {
        // Linux'ta dosya adı satır sonu içerebilir. Bu mesaj günlüğe yazılıyor;
        // kaçış olmadan sahte bir kayıt satırı üretilebilirdi.
        let err = OpError::io("/tmp/Save\n[2026-08-17] ERROR sahte")(std::io::Error::other("x"));
        let text = err.to_string();
        assert!(!text.contains('\n'), "satır bölünüyor: {text}");
        assert!(text.contains("\\n"), "{text}");
    }

    #[test]
    fn an_unsafe_archive_name_with_a_newline_is_escaped() {
        // Bu dize tümüyle arşivi üretenin denetiminde.
        let err = OpError::UnsafePath("evil\n[2026-08-17] INFO sahte".to_string());
        assert!(!err.to_string().contains('\n'), "{err}");
    }

    #[test]
    fn conflicts_lists_every_clashing_name() {
        let err = OpError::Conflicts(vec!["SaveA".to_string(), "SaveB".to_string()]);
        assert!(err.to_string().contains("SaveA, SaveB"), "{err}");
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
