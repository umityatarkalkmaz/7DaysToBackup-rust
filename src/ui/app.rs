//! Ana pencere.
//!
//! `window.py`'nin karşılığı, ama immediate-mode. Widget'lar kalıcı nesne
//! olmadığı için Python'daki `_retranslate_ui()` adımı yok; buna karşılık
//! "her karede diski tarama" sorumluluğu bize geçiyor (bkz. [`BackupApp::rescan_maps`]).

use eframe::CreationContext;
use egui::{RichText, Ui};
use std::path::{Path, PathBuf};

use crate::core::config::Config;
use crate::core::paths;
use crate::core::platform;
use crate::i18n::{Lang, Strings, fill1};
use crate::ui::theme;

pub struct BackupApp {
    config_path: PathBuf,
    config: Config,
    lang: Lang,

    maps: Vec<String>,
    selected_map: Option<usize>,
    saves: Vec<String>,
    selected_save: Option<usize>,

    /// Save klasörü bulunamadığında gösterilen açıklama.
    ///
    /// Python bunu bilinçli olarak modal yerine satır içi bir etikette
    /// gösteriyor: açılış, kullanıcının henüz üzerinde işlem yapamayacağı bir
    /// pencereyle bloke edilmemeli.
    status: Option<String>,
}

impl BackupApp {
    pub fn new(cc: &CreationContext<'_>) -> Self {
        cc.egui_ctx.set_visuals(theme::dark_visuals());

        let config_path = paths::config_file();
        let config = Config::load(&config_path);
        let lang = Lang::from_code(&config.language);

        let mut app = Self {
            config_path,
            config,
            lang,
            maps: Vec::new(),
            selected_map: None,
            saves: Vec::new(),
            selected_save: None,
            status: None,
        };
        app.rescan_maps();
        app
    }

    fn strings(&self) -> &'static Strings {
        self.lang.strings()
    }

    // --------------------------------------------------------------- listeler

    /// Map listesini yeniden tarar.
    ///
    /// **Bu çizim fonksiyonundan çağrılmaz.** egui her kareyi yeniden çizdiği
    /// için oradan çağırmak saniyede 60 kez `read_dir` demek olurdu; ağ
    /// sürücüsündeki bir save klasöründe uygulama kilitlenirdi. Listeler durumda
    /// tutulur ve yalnızca şu anlarda tazelenir: açılış, ayar değişikliği, map
    /// seçimi ve biten bir işlem.
    pub fn rescan_maps(&mut self) {
        let saves_path = platform::saves_path(&self.config);

        if !saves_path.is_dir() {
            self.status = Some(fill1(self.strings().saves_missing, saves_path.display()));
            self.maps.clear();
            self.saves.clear();
            self.selected_map = None;
            self.selected_save = None;
            return;
        }

        self.status = None;
        self.maps = read_dir_names(&saves_path);
        self.selected_map = None;
        self.saves.clear();
        self.selected_save = None;
    }

    /// Seçili map'in save listesini tazeler.
    pub fn rescan_saves(&mut self) {
        self.saves.clear();
        self.selected_save = None;

        let Some(map) = self.selected_map_name() else {
            return;
        };
        let path = platform::saves_path(&self.config).join(map);
        if path.is_dir() {
            self.saves = read_dir_names(&path);
        }
    }

    fn selected_map_name(&self) -> Option<&str> {
        self.selected_map
            .and_then(|index| self.maps.get(index))
            .map(String::as_str)
    }

    fn selected_save_name(&self) -> Option<&str> {
        self.selected_save
            .and_then(|index| self.saves.get(index))
            .map(String::as_str)
    }

    /// Seçili save'in tam yolu. İkisinden biri seçili değilse `None`.
    fn selected_save_path(&self) -> Option<PathBuf> {
        let map = self.selected_map_name()?;
        let save = self.selected_save_name()?;
        Some(platform::saves_path(&self.config).join(map).join(save))
    }

    // ------------------------------------------------------------------ çizim

    fn header(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            let settings = self.strings().settings;
            if ui.button(RichText::new(settings).size(16.0)).clicked() {
                // Faz 5'te ayarlar penceresini açacak.
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut chosen = self.lang;
                egui::ComboBox::from_id_salt("language")
                    .selected_text(self.lang.display())
                    .show_ui(ui, |ui| {
                        for lang in Lang::ALL {
                            ui.selectable_value(&mut chosen, lang, lang.display());
                        }
                    });
                if chosen != self.lang {
                    self.set_language(chosen);
                }
            });
        });
    }

    fn set_language(&mut self, lang: Lang) {
        self.lang = lang;
        self.config.language = lang.code().to_string();
        if let Err(error) = self.config.save(&self.config_path) {
            log::warn!("Dil ayarı kaydedilemedi: {error}");
        }
        // Yalnızca hata metni dile bağlı olduğu için tazeleniyor; Python burada
        // bütün map listesini yeniden okuyor, buna gerek yok.
        if self.status.is_some() {
            let saves_path = platform::saves_path(&self.config);
            self.status = Some(fill1(self.strings().saves_missing, saves_path.display()));
        }
    }

    fn lists(&mut self, ui: &mut Ui) {
        let strings = self.strings();
        let mut map_changed = false;

        ui.columns(2, |columns| {
            columns[0].label(strings.map_list);
            let before = self.selected_map;
            selectable_list(&mut columns[0], "maps", &self.maps, &mut self.selected_map);
            map_changed = before != self.selected_map;

            columns[1].label(strings.save_list);
            selectable_list(
                &mut columns[1],
                "saves",
                &self.saves,
                &mut self.selected_save,
            );
        });

        if map_changed {
            self.rescan_saves();
        }
    }

    fn actions(&mut self, ui: &mut Ui) {
        let strings = self.strings();
        let has_save = self.selected_save_path().is_some();
        let has_map = self.selected_map_name().is_some();

        ui.add_space(4.0);
        ui.vertical_centered_justified(|ui| {
            if ui
                .add_enabled(has_save, egui::Button::new(strings.backup))
                .clicked()
            {
                // Faz 5
            }
            if ui
                .add_enabled(has_save, egui::Button::new(strings.delete))
                .clicked()
            {
                // Faz 5
            }
        });

        ui.add_space(4.0);
        ui.columns(2, |columns| {
            let _ = columns[0].add_enabled(has_save, egui::Button::new(strings.export));
            let _ = columns[1].add_enabled(has_map, egui::Button::new(strings.import));
        });
    }
}

impl eframe::App for BackupApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        egui::Panel::top("header").show(ui, |ui| {
            ui.add_space(4.0);
            self.header(ui);
            ui.add_space(4.0);
        });

        if let Some(status) = self.status.clone() {
            egui::Panel::bottom("status").show(ui, |ui| {
                ui.add_space(4.0);
                ui.label(RichText::new(status).color(theme::STATUS_TEXT));
                ui.add_space(4.0);
            });
        }

        egui::CentralPanel::default().show(ui, |ui| {
            self.lists(ui);
            self.actions(ui);
        });
    }
}

/// Bir dizindeki **alt dizinlerin** adlarını sıralı döndürür.
///
/// **Python'dan küçük bir sapma.** Orada map listesi `entry.is_dir()` ile
/// süzülüyor ama save listesi süzülmüyordu. Save'ler her zaman dizindir ve
/// bütün işlemler bunu varsayar; süzülmemiş bir listede save klasörüne düşmüş
/// başıboş bir dosya save gibi görünür ve "yedekle" dendiğinde ortaya boş bir
/// dizin çıkardı — o da bir sonraki taramada gerçek bir save gibi listelenirdi.
fn read_dir_names(path: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(path) else {
        return Vec::new();
    };

    let mut names: Vec<String> = entries
        .flatten()
        // `file_type()` dizin girdisinin kendisinden okunur; girdi başına ayrı
        // bir `stat` çağrısı gerekmez (Python'un `os.scandir` tercihiyle aynı).
        .filter(|entry| entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false))
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

fn selectable_list(ui: &mut Ui, id: &str, items: &[String], selected: &mut Option<usize>) {
    egui::Frame::new()
        .fill(ui.visuals().extreme_bg_color)
        .inner_margin(4.0)
        .show(ui, |ui| {
            egui::ScrollArea::vertical()
                .id_salt(id)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    for (index, item) in items.iter().enumerate() {
                        if ui
                            .selectable_label(*selected == Some(index), item)
                            .clicked()
                        {
                            *selected = Some(index);
                        }
                    }
                });
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_dir_names_lists_only_directories_sorted() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("Navezgane")).unwrap();
        std::fs::create_dir(dir.path().join("Alpha County")).unwrap();
        std::fs::write(dir.path().join("stray.txt"), b"x").unwrap();

        assert_eq!(
            read_dir_names(dir.path()),
            vec!["Alpha County".to_string(), "Navezgane".to_string()]
        );
    }

    #[test]
    fn read_dir_names_is_empty_for_a_missing_directory() {
        let dir = tempfile::tempdir().unwrap();
        assert!(read_dir_names(&dir.path().join("nope")).is_empty());
    }
}
