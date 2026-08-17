//! Ayarlar penceresi — `settings_dialog.py` karşılığı.

use egui::Ui;
use std::path::Path;

use crate::core::config::Config;
use crate::core::platform;
use crate::i18n::{Strings, fill1};

/// Ayarlar penceresinin genişliği.
const WIDTH: f32 = 520.0;

/// Ayarlar penceresinin taslak durumu.
///
/// Değişiklikler doğrudan [`Config`] üzerine değil, buraya yazılır. Kullanıcı
/// İptal derse hiçbir şey değişmemiş olur.
pub struct SettingsState {
    custom_save_path: String,
    /// Taslak arayüz ölçeği. `None` = otomatik.
    ///
    /// Kaydırıcı `Option` üzerinde çalışamadığı için ayrı bir onay kutusuyla
    /// yönetiliyor; kutu işaretliyken değer diske `0.0` olarak yazılır ve
    /// [`Config::ui_scale`] onu otomatik diye okur.
    ui_scale: Option<f32>,
    /// Otomatik yedekleme aralığı, dakika. 0 = kapalı.
    auto_backup_minutes: u32,
    /// Saklanacak yedek sayısı. 0 = sınırsız.
    auto_backup_keep: u32,
    /// Kaydetme başarısız olduğunda pencerede gösterilen mesaj.
    error: Option<String>,
}

/// Pencerenin bu karedeki sonucu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsOutcome {
    /// Açık kalmaya devam ediyor.
    Open,
    /// Kaydedildi ve kapandı — çağıran listeleri tazelemeli.
    Saved,
    /// Değişiklik yapılmadan kapandı.
    Dismissed,
}

impl SettingsState {
    pub fn new(config: &Config) -> Self {
        Self {
            custom_save_path: config.custom_save_path.clone(),
            ui_scale: config.ui_scale(),
            auto_backup_minutes: config.auto_backup_minutes,
            auto_backup_keep: config.auto_backup_keep,
            error: None,
        }
    }

    pub fn show(
        &mut self,
        ui: &mut Ui,
        strings: &Strings,
        config: &mut Config,
        config_path: &Path,
    ) -> SettingsOutcome {
        let mut outcome = SettingsOutcome::Open;

        // `set_width` (asgari değil, sabit): yalnızca asgari verildiğinde uzun
        // Türkçe yardım metni pencereyi büyütmüyor, sağdan kırpılıyordu.
        ui.set_width(WIDTH);
        ui.heading(strings.settings_title);
        ui.add_space(8.0);

        ui.group(|ui| {
            ui.set_width(ui.available_width());
            ui.label(strings.advanced_settings);
            ui.add_space(4.0);
            ui.label(strings.custom_save_path_label);

            ui.horizontal(|ui| {
                // Genişlik kalan alandan hesaplanıyor: sabit bir değer, iki
                // düğmenin metni uzadığında (dil değişince) taşmaya yol açar.
                let buttons = 150.0;
                ui.add(
                    egui::TextEdit::singleline(&mut self.custom_save_path)
                        .hint_text(strings.custom_save_path_placeholder)
                        .desired_width((ui.available_width() - buttons).max(120.0)),
                );

                if ui.button("...").clicked()
                    && let Some(folder) = rfd::FileDialog::new()
                        .set_title(strings.select_folder)
                        .pick_folder()
                {
                    self.custom_save_path = folder.to_string_lossy().into_owned();
                }

                if ui.button(strings.open_folder).clicked() {
                    self.open_folder(strings, config);
                }
            });

            ui.add_space(4.0);
            ui.label(egui::RichText::new(strings.custom_save_path_help).weak());
        });

        ui.add_space(8.0);
        ui.group(|ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(strings.auto_backup_label);
                ui.add(egui::DragValue::new(&mut self.auto_backup_minutes).range(0..=1440));
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(strings.auto_backup_keep_label);
                ui.add(egui::DragValue::new(&mut self.auto_backup_keep).range(0..=100));
            });
            ui.add_space(4.0);
            ui.label(egui::RichText::new(strings.auto_backup_help).weak());
        });

        ui.add_space(8.0);
        ui.group(|ui| {
            ui.set_width(ui.available_width());
            ui.label(strings.ui_scale_label);
            ui.add_space(4.0);

            ui.horizontal(|ui| {
                let mut automatic = self.ui_scale.is_none();
                if ui.checkbox(&mut automatic, strings.ui_scale_auto).changed() {
                    // Otomatikten çıkarken kaydırıcı, o an geçerli olan ölçekten
                    // başlasın; 1.0'a atlamak kullanıcıya bir geri adım gibi gelir.
                    self.ui_scale = if automatic {
                        None
                    } else {
                        Some(ui.ctx().zoom_factor())
                    };
                }

                if let Some(scale) = self.ui_scale.as_mut() {
                    ui.add(egui::Slider::new(scale, 0.75..=2.0).step_by(0.05));
                }
            });

            ui.add_space(4.0);
            ui.label(egui::RichText::new(strings.ui_scale_help).weak());
        });

        if let Some(error) = &self.error {
            ui.add_space(6.0);
            ui.label(egui::RichText::new(error).color(super::theme::STATUS_TEXT));
        }

        ui.add_space(10.0);
        ui.horizontal(|ui| {
            if ui.button(strings.save).clicked() {
                outcome = self.save(config, config_path, strings);
            }
            if ui.button(strings.cancel).clicked() {
                outcome = SettingsOutcome::Dismissed;
            }
        });

        outcome
    }

    /// Ayarları yazar.
    ///
    /// Yazma başarısız olursa pencere **kapanmaz**. Python sürümündeki karar:
    /// kaydedilmemiş bir ayarı kaydedilmiş gibi kapatmak sessiz veri kaybıdır.
    fn save(
        &mut self,
        config: &mut Config,
        config_path: &Path,
        strings: &Strings,
    ) -> SettingsOutcome {
        let path = self.custom_save_path.trim().to_string();
        // 0.0 = otomatik; `Config::ui_scale` sözleşmesi bu.
        let scale = self.ui_scale.unwrap_or(0.0);
        let previous = std::mem::replace(&mut config.custom_save_path, path.clone());
        let previous_scale = std::mem::replace(&mut config.ui_scale, scale);
        let previous_minutes =
            std::mem::replace(&mut config.auto_backup_minutes, self.auto_backup_minutes);
        let previous_keep = std::mem::replace(&mut config.auto_backup_keep, self.auto_backup_keep);

        match config.save(config_path) {
            Ok(()) => {
                log::info!(
                    "Ayarlar kaydedildi. custom_save_path={path:?} ui_scale={scale} \
                     auto_backup_minutes={} auto_backup_keep={}",
                    self.auto_backup_minutes,
                    self.auto_backup_keep
                );
                self.error = None;
                SettingsOutcome::Saved
            }
            Err(error) => {
                // Bellekteki değeri de geri al: diske gitmeyen bir ayar,
                // uygulamanın geri kalanında yürürlükte görünmemeli.
                config.custom_save_path = previous;
                config.ui_scale = previous_scale;
                config.auto_backup_minutes = previous_minutes;
                config.auto_backup_keep = previous_keep;
                log::error!("Ayarlar kaydedilemedi: {error}");
                self.error = Some(strings.settings_save_failed.to_string());
                SettingsOutcome::Open
            }
        }
    }

    /// Save klasörünü sistem dosya yöneticisinde açar.
    ///
    /// Python burada `os.startfile` / `xdg-open` / `open` üçlüsünü elle
    /// ayırıyordu; `open` crate'i aynı işi tek çağrıda yapar.
    fn open_folder(&mut self, strings: &Strings, config: &Config) {
        let typed = self.custom_save_path.trim();
        let path = if !typed.is_empty() && Path::new(typed).is_dir() {
            std::path::PathBuf::from(typed)
        } else {
            platform::saves_path(config)
        };

        if !path.exists() {
            log::warn!("Klasör açılamadı, yol yok: {path:?}");
            self.error = Some(fill1(strings.path_not_found, path.display()));
            return;
        }

        // Dosya yöneticisini başlatmak pek çok şekilde başarısız olabilir
        // (xdg-open yok, masaüstü oturumu yok, işletim sistemi reddediyor);
        // hepsi kullanıcıya bildirilir, pencere çökmez.
        match open::that_detached(&path) {
            Ok(()) => log::info!("Klasör açıldı: {path:?}"),
            Err(error) => {
                log::error!("Klasör açılamadı: {error}");
                self.error = Some(fill1(strings.open_failed, error));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::Lang;

    #[test]
    fn starts_from_the_current_configuration() {
        let config = Config {
            custom_save_path: "/tmp/saves".to_string(),
            language: "en".to_string(),
            ..Default::default()
        };
        assert_eq!(SettingsState::new(&config).custom_save_path, "/tmp/saves");
    }

    #[test]
    fn saving_trims_and_persists() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        let mut config = Config::default();
        let mut state = SettingsState::new(&config);
        state.custom_save_path = "  /tmp/saves  ".to_string();

        let outcome = state.save(&mut config, &path, Lang::Tr.strings());

        assert_eq!(outcome, SettingsOutcome::Saved);
        assert_eq!(config.custom_save_path, "/tmp/saves");
        assert_eq!(Config::load(&path).custom_save_path, "/tmp/saves");
    }

    #[test]
    fn a_failed_save_keeps_the_dialog_open_and_rolls_back() {
        // Üst dizinin yerinde bir dosya var; yazma başarısız olmalı.
        let dir = tempfile::tempdir().unwrap();
        let blocker = dir.path().join("blocker");
        std::fs::write(&blocker, b"not a directory").unwrap();

        let mut config = Config {
            custom_save_path: "/original".to_string(),
            language: String::new(),
            ..Default::default()
        };
        let mut state = SettingsState::new(&config);
        state.custom_save_path = "/new".to_string();

        let outcome = state.save(
            &mut config,
            &blocker.join("config.json"),
            Lang::Tr.strings(),
        );

        assert_eq!(outcome, SettingsOutcome::Open);
        assert!(state.error.is_some());
        // Diske gitmeyen ayar bellekte de yürürlüğe girmemeli.
        assert_eq!(config.custom_save_path, "/original");
    }

    #[test]
    fn an_empty_path_is_stored_as_auto_detect() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        let mut config = Config {
            custom_save_path: "/tmp/old".to_string(),
            language: String::new(),
            ..Default::default()
        };
        let mut state = SettingsState::new(&config);
        state.custom_save_path = "   ".to_string();

        assert_eq!(
            state.save(&mut config, &path, Lang::Tr.strings()),
            SettingsOutcome::Saved
        );
        assert_eq!(config.custom_save_path, "");
    }
}
