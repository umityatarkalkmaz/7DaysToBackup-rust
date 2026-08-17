//! Arayüz metinleri.
//!
//! Python'da bu bir sözlükler sözlüğüydü ve her erişim (`LANGUAGES[code][key]`)
//! çalışma zamanında `KeyError` verebiliyordu. Nitekim geçmişte `_retranslate_ui`
//! ayarlar düğmesinin metnini atlamıştı. Burada her metin bir struct alanı:
//! eksik bir alan derleme hatasıdır.
//!
//! Immediate-mode sayesinde `_retranslate_ui()` diye bir adım da yok — dil
//! değişince bir sonraki kare zaten yeni [`Strings`]'i okur.

use std::fmt::Display;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Lang {
    #[default]
    Tr,
    En,
}

impl Lang {
    pub const ALL: [Lang; 2] = [Lang::Tr, Lang::En];

    /// Yapılandırmada saklanan kod.
    pub fn code(self) -> &'static str {
        match self {
            Lang::Tr => "tr",
            Lang::En => "en",
        }
    }

    /// Dil seçicide gösterilen ad.
    pub fn display(self) -> &'static str {
        match self {
            Lang::Tr => "Türkçe",
            Lang::En => "English",
        }
    }

    /// Bilinmeyen kod varsayılana düşer — Python da öyle yapıyor.
    pub fn from_code(code: &str) -> Lang {
        Lang::ALL
            .into_iter()
            .find(|lang| lang.code() == code)
            .unwrap_or_default()
    }

    pub fn strings(self) -> &'static Strings {
        match self {
            Lang::Tr => &TR,
            Lang::En => &EN,
        }
    }
}

/// Tek bir dilin bütün metinleri.
///
/// `{}` içeren alanlar şablondur; [`fill1`] ile doldurulur.
///
/// Dört `*_error` alanı bilerek yer tutucusuz: bunlar bir önek ve arkalarına
/// hatanın kendisi ekleniyor. Python'da `export_error` hem `{}` içeriyor hem de
/// `.format("")` ile çağrılıp üstüne `" - {mesaj}"` ekleniyordu; sonuç
/// "hata oluştu:  - mesaj" gibi çift noktalamalı bir metindi.
#[derive(Debug)]
pub struct Strings {
    pub title: &'static str,
    pub map_list: &'static str,
    pub save_list: &'static str,
    pub backup: &'static str,
    pub delete: &'static str,
    pub export: &'static str,
    pub import: &'static str,
    pub backup_success: &'static str,
    pub backup_error: &'static str,
    pub delete_success: &'static str,
    pub delete_error: &'static str,
    pub export_success: &'static str,
    pub export_error: &'static str,
    pub import_success: &'static str,
    pub import_error: &'static str,
    pub delete_confirm: &'static str,
    pub import_exists: &'static str,
    pub import_select: &'static str,
    pub selection_error: &'static str,
    pub saves_missing: &'static str,
    pub settings: &'static str,
    pub settings_title: &'static str,
    pub advanced_settings: &'static str,
    pub custom_save_path_label: &'static str,
    pub custom_save_path_placeholder: &'static str,
    pub custom_save_path_help: &'static str,
    pub ui_scale_label: &'static str,
    pub ui_scale_auto: &'static str,
    pub ui_scale_help: &'static str,
    pub select_folder: &'static str,
    pub save: &'static str,
    pub cancel: &'static str,
    pub close: &'static str,
    pub open_folder: &'static str,
    pub path_not_found: &'static str,
    pub open_failed: &'static str,
    pub error: &'static str,
    pub settings_save_failed: &'static str,
    pub backup_progress: &'static str,
    pub delete_progress: &'static str,
    pub export_progress: &'static str,
    pub import_progress: &'static str,
    pub cancelled: &'static str,
    pub confirm_yes: &'static str,
    pub confirm_no: &'static str,
}

pub static TR: Strings = Strings {
    title: "7 Days To Die Save Yedekleme Aracı",
    map_list: "Map Listesi",
    save_list: "Save Listesi",
    backup: "Yedekle",
    delete: "Sil",
    export: "Dışa Aktar",
    import: "İçe Aktar",
    backup_success: "Yedekleme başarılı",
    backup_error: "Yedekleme sırasında bir hata oluştu",
    delete_success: "Silme başarılı",
    delete_error: "Silme sırasında bir hata oluştu",
    export_success: "Dışa aktarma tamamlandı: {}",
    export_error: "Dışa aktarma sırasında hata oluştu",
    import_success: "İçe aktarma tamamlandı.",
    import_error: "İçe aktarma sırasında hata oluştu",
    delete_confirm: "'{}' dosyasını silmek istediğinize emin misiniz?",
    import_exists: "Aynı isimde bir save zaten var!",
    import_select: "Zip dosyası seç",
    selection_error: "Lütfen bir map ve save seçin.",
    saves_missing: "Save klasörü bulunamadı!\n\nOlası nedenler:\n• 7 Days to Die oyunu yüklü olmayabilir\n• Oyunu henüz hiç oynamamış olabilirsiniz\n• Save dosyalarınız farklı bir konumda olabilir\n\nAranan konum:\n{}",
    settings: "⚙",
    settings_title: "Ayarlar",
    advanced_settings: "Gelişmiş Ayarlar",
    custom_save_path_label: "Özel Save Dosyası Konumu (İsteğe Bağlı):",
    custom_save_path_placeholder: "Otomatik algılama için boş bırakın",
    custom_save_path_help: "Not: Bu ayar sadece oyun save dosyalarını otomatik bulamazsa kullanılmalıdır.",
    ui_scale_label: "Arayüz ölçeği",
    ui_scale_auto: "Otomatik",
    ui_scale_help: "Arayüz ekranınızda küçük görünüyorsa bu değeri artırın. Ctrl ve +/- tuşları da çalışır; buradaki değer kalıcıdır.",
    select_folder: "Klasör Seç",
    save: "Kaydet",
    cancel: "İptal",
    close: "Kapat",
    open_folder: "Klasörü Aç",
    path_not_found: "Klasör bulunamadı: {}",
    open_failed: "Klasör açılamadı: {}",
    error: "Hata",
    settings_save_failed: "Ayarlar kaydedilemedi. Ayrıntılar için günlük dosyasına bakın.",
    backup_progress: "Yedekleniyor...",
    delete_progress: "Siliniyor...",
    export_progress: "Dışa aktarılıyor...",
    import_progress: "İçe aktarılıyor...",
    cancelled: "İşlem iptal edildi.",
    confirm_yes: "Evet",
    confirm_no: "Hayır",
};

pub static EN: Strings = Strings {
    title: "7 Days To Die Save Backup Tool",
    map_list: "Map List",
    save_list: "Save List",
    backup: "Backup",
    delete: "Delete",
    export: "Export",
    import: "Import",
    backup_success: "Backup successful",
    backup_error: "An error occurred during backup",
    delete_success: "Delete successful",
    delete_error: "An error occurred during delete",
    export_success: "Export completed: {}",
    export_error: "An error occurred during export",
    import_success: "Import completed.",
    import_error: "An error occurred during import",
    delete_confirm: "Are you sure you want to delete '{}' save?",
    import_exists: "A save with the same name already exists!",
    import_select: "Select zip file",
    selection_error: "Please select a map and save.",
    saves_missing: "Save folder not found!\n\nPossible reasons:\n• 7 Days to Die may not be installed\n• You may not have played the game yet\n• Your save files may be in a different location\n\nSearched location:\n{}",
    settings: "⚙",
    settings_title: "Settings",
    advanced_settings: "Advanced Settings",
    custom_save_path_label: "Custom Save Folder Location (Optional):",
    custom_save_path_placeholder: "Leave empty for auto-detection",
    custom_save_path_help: "Note: Use this setting only if the game cannot automatically find save files.",
    ui_scale_label: "Interface scale",
    ui_scale_auto: "Automatic",
    ui_scale_help: "Increase this if the interface looks small on your display. Ctrl and +/- work too; this value is the one that persists.",
    select_folder: "Select Folder",
    save: "Save",
    cancel: "Cancel",
    close: "Close",
    open_folder: "Open Folder",
    path_not_found: "Folder not found: {}",
    open_failed: "Failed to open folder: {}",
    error: "Error",
    settings_save_failed: "Could not save settings. See the log file for details.",
    backup_progress: "Backing up...",
    delete_progress: "Deleting...",
    export_progress: "Exporting...",
    import_progress: "Importing...",
    cancelled: "Operation cancelled.",
    confirm_yes: "Yes",
    confirm_no: "No",
};

/// Şablondaki ilk `{}` yerine `value` koyar — Python'un `str.format(arg)` karşılığı.
///
/// Şablonda `{}` yoksa metin olduğu gibi döner: eksik bir yer tutucu, kullanıcıya
/// gösterilecek metni bozmaktansa sessizce yoksayılır.
pub fn fill1(template: &str, value: impl Display) -> String {
    match template.find("{}") {
        Some(index) => format!("{}{}{}", &template[..index], value, &template[index + 2..]),
        None => template.to_string(),
    }
}

#[cfg(test)]
impl Strings {
    /// Testlerin bütün alanları tek tek gezebilmesi için (ad, değer) çiftleri.
    fn fields(&self) -> Vec<(&'static str, &'static str)> {
        vec![
            ("title", self.title),
            ("map_list", self.map_list),
            ("save_list", self.save_list),
            ("backup", self.backup),
            ("delete", self.delete),
            ("export", self.export),
            ("import", self.import),
            ("backup_success", self.backup_success),
            ("backup_error", self.backup_error),
            ("delete_success", self.delete_success),
            ("delete_error", self.delete_error),
            ("export_success", self.export_success),
            ("export_error", self.export_error),
            ("import_success", self.import_success),
            ("import_error", self.import_error),
            ("delete_confirm", self.delete_confirm),
            ("import_exists", self.import_exists),
            ("import_select", self.import_select),
            ("selection_error", self.selection_error),
            ("saves_missing", self.saves_missing),
            ("settings", self.settings),
            ("settings_title", self.settings_title),
            ("ui_scale_label", self.ui_scale_label),
            ("ui_scale_auto", self.ui_scale_auto),
            ("ui_scale_help", self.ui_scale_help),
            ("advanced_settings", self.advanced_settings),
            ("custom_save_path_label", self.custom_save_path_label),
            (
                "custom_save_path_placeholder",
                self.custom_save_path_placeholder,
            ),
            ("custom_save_path_help", self.custom_save_path_help),
            ("select_folder", self.select_folder),
            ("save", self.save),
            ("cancel", self.cancel),
            ("close", self.close),
            ("open_folder", self.open_folder),
            ("path_not_found", self.path_not_found),
            ("open_failed", self.open_failed),
            ("error", self.error),
            ("settings_save_failed", self.settings_save_failed),
            ("backup_progress", self.backup_progress),
            ("delete_progress", self.delete_progress),
            ("export_progress", self.export_progress),
            ("import_progress", self.import_progress),
            ("cancelled", self.cancelled),
            ("confirm_yes", self.confirm_yes),
            ("confirm_no", self.confirm_no),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_string_is_empty() {
        for lang in Lang::ALL {
            for (name, value) in lang.strings().fields() {
                assert!(!value.trim().is_empty(), "{}.{name} boş", lang.code());
            }
        }
    }

    #[test]
    fn placeholders_match_across_languages() {
        // Bir dilde `{}` olup diğerinde olmaması, o dilde yolun/adın hiç
        // görünmemesi demektir. Python'da bunu yakalayacak bir şey yoktu.
        for ((name, turkish), (_, english)) in TR.fields().into_iter().zip(EN.fields()) {
            assert_eq!(
                turkish.matches("{}").count(),
                english.matches("{}").count(),
                "'{name}' alanında yer tutucu sayısı tutmuyor"
            );
        }
    }

    #[test]
    fn language_codes_round_trip() {
        for lang in Lang::ALL {
            assert_eq!(Lang::from_code(lang.code()), lang);
        }
    }

    #[test]
    fn an_unknown_code_falls_back_to_the_default() {
        assert_eq!(Lang::from_code("de"), Lang::default());
        assert_eq!(Lang::from_code(""), Lang::default());
    }

    #[test]
    fn fill1_substitutes_the_first_placeholder() {
        assert_eq!(
            fill1("Tamamlandı: {}", "/tmp/a.zip"),
            "Tamamlandı: /tmp/a.zip"
        );
        assert_eq!(fill1("'{}' silinsin mi?", "SaveA"), "'SaveA' silinsin mi?");
    }

    #[test]
    fn fill1_leaves_a_template_without_a_placeholder_alone() {
        assert_eq!(
            fill1("Yedekleme başarılı", "yok sayılır"),
            "Yedekleme başarılı"
        );
    }

    #[test]
    fn fill1_handles_multibyte_text_around_the_placeholder() {
        // Dilim sınırları bayt indeksli; Türkçe karakterlerde panik olmamalı.
        assert_eq!(fill1("Şğüöç {} İçerik", "X"), "Şğüöç X İçerik");
    }
}
