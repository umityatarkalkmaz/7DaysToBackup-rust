//! Ana pencere.
//!
//! `window.py`'nin karşılığı, ama immediate-mode. Widget'lar kalıcı nesne
//! olmadığı için Python'daki `_retranslate_ui()` adımı yok; buna karşılık
//! "her karede diski tarama" sorumluluğu bize geçiyor (bkz. [`BackupApp::rescan_maps`]).

use eframe::CreationContext;
use egui::{Context, Id, RichText, Ui};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::core::config::Config;
use crate::core::ops::{self, PathKind};
use crate::core::paths;
use crate::core::platform;
use crate::i18n::{Lang, Strings, fill1};
use crate::task::{self, Outcome, TaskHandle, TaskKind};
use crate::ui::settings::{SettingsOutcome, SettingsState};
use crate::ui::theme;

/// Onaylandığında çalıştırılacak iş.
enum PendingAction {
    Delete(PathBuf),
}

/// Aynı anda en fazla biri açık olabilen pencereler.
enum Dialog {
    Info(String),
    Error(String),
    Confirm { text: String, action: PendingAction },
    Settings(Box<SettingsState>),
}

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

    task: Option<TaskHandle>,
    dialog: Option<Dialog>,

    /// En son dışa aktarmanın hedefi.
    ///
    /// Başarı mesajı dosyanın nereye yazıldığını söylemeli, ama yol işlemin
    /// başlangıcında hesaplanıyor ve iş parçacığına taşınıyor; sonucu bildirirken
    /// geri okunabilmesi için burada tutuluyor.
    last_export: Option<PathBuf>,
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
            task: None,
            dialog: None,
            last_export: None,
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
        let previous = self.selected_map_name().map(str::to_string);
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
        self.selected_map = previous.and_then(|name| index_of(&self.maps, &name));
        self.rescan_saves();
    }

    /// Seçili map'in save listesini tazeler.
    ///
    /// Seçim adıyla korunur. Python `finish()` içinde `load_saves()` çağırıp
    /// seçimi düşürüyordu; yedekten sonra kullanıcının aynı save'i yeniden
    /// seçmesi gerekiyordu. Silmeden sonra ad zaten kaybolduğu için seçim
    /// kendiliğinden temizlenir.
    pub fn rescan_saves(&mut self) {
        let previous = self.selected_save_name().map(str::to_string);
        self.saves.clear();
        self.selected_save = None;

        let Some(map) = self.selected_map_name() else {
            return;
        };
        let path = platform::saves_path(&self.config).join(map);
        if path.is_dir() {
            self.saves = read_dir_names(&path);
        }
        self.selected_save = previous.and_then(|name| index_of(&self.saves, &name));
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

    // ------------------------------------------------------------- işlem metni

    fn progress_text(&self, kind: TaskKind) -> &'static str {
        let strings = self.strings();
        match kind {
            TaskKind::Backup => strings.backup_progress,
            TaskKind::Delete => strings.delete_progress,
            TaskKind::Export => strings.export_progress,
            TaskKind::Import => strings.import_progress,
        }
    }

    fn error_text(&self, kind: TaskKind) -> &'static str {
        let strings = self.strings();
        match kind {
            TaskKind::Backup => strings.backup_error,
            TaskKind::Delete => strings.delete_error,
            TaskKind::Export => strings.export_error,
            TaskKind::Import => strings.import_error,
        }
    }

    // ---------------------------------------------------------------- işlemler

    fn poll_task(&mut self) {
        let Some(handle) = self.task.as_mut() else {
            return;
        };
        let Some(outcome) = handle.poll() else {
            return;
        };
        let kind = handle.kind();

        self.task = None;
        self.rescan_saves();

        let strings = self.strings();
        self.dialog = Some(match outcome {
            Outcome::Success(TaskKind::Backup) => Dialog::Info(strings.backup_success.to_string()),
            Outcome::Success(TaskKind::Delete) => Dialog::Info(strings.delete_success.to_string()),
            Outcome::Success(TaskKind::Export) => Dialog::Info(match &self.last_export {
                Some(path) => fill1(strings.export_success, path.display()),
                None => fill1(strings.export_success, ""),
            }),
            Outcome::Success(TaskKind::Import) => Dialog::Info(strings.import_success.to_string()),
            Outcome::Cancelled => Dialog::Info(strings.cancelled.to_string()),
            Outcome::Error(message) => {
                log::error!("{}: {message}", self.error_text(kind));
                Dialog::Error(format!("{}\n\n{message}", self.error_text(kind)))
            }
        });
    }

    fn start(
        &mut self,
        ctx: &Context,
        kind: TaskKind,
        cancellable: bool,
        work: impl FnOnce(&dyn ops::ProgressSink) -> Result<(), crate::core::OpError> + Send + 'static,
    ) {
        self.task = Some(task::spawn(kind, cancellable, wake(ctx), work));
    }

    fn require_selection(&mut self) -> Option<PathBuf> {
        match self.selected_save_path() {
            Some(path) => Some(path),
            None => {
                self.dialog = Some(Dialog::Error(self.strings().selection_error.to_string()));
                None
            }
        }
    }

    fn on_backup(&mut self, ctx: &Context) {
        let Some(source) = self.require_selection() else {
            return;
        };

        // `OsString` üzerinden ekleniyor: `display()` üzerinden geçmek UTF-8
        // olmayan bir dosya adını sessizce bozardı.
        let mut name = source.clone().into_os_string();
        name.push(format!("_backup_{}", ops::timestamp_suffix()));
        let destination = ops::unique_path(PathBuf::from(name), PathKind::Dir);

        log::info!("Yedek {} -> {}", source.display(), destination.display());
        self.start(ctx, TaskKind::Backup, true, move |sink| {
            ops::copy_save(&source, &destination, sink)
        });
    }

    fn on_delete(&mut self, _ctx: &Context) {
        let Some(source) = self.require_selection() else {
            return;
        };
        let name = self.selected_save_name().unwrap_or_default().to_string();
        self.dialog = Some(Dialog::Confirm {
            text: fill1(self.strings().delete_confirm, name),
            action: PendingAction::Delete(source),
        });
    }

    fn on_export(&mut self, ctx: &Context) {
        let Some(source) = self.require_selection() else {
            return;
        };
        let name = self.selected_save_name().unwrap_or_default().to_string();

        let zip_path = ops::unique_path(
            platform::export_dir().join(format!("{name}_{}.zip", ops::timestamp_suffix())),
            PathKind::File,
        );

        log::info!("Dışa aktar {} -> {}", source.display(), zip_path.display());
        self.last_export = Some(zip_path.clone());
        self.start(ctx, TaskKind::Export, true, move |sink| {
            ops::export_save(&source, &zip_path, ops::DEFAULT_COMPRESSION_LEVEL, sink)
        });
    }

    fn on_import(&mut self, ctx: &Context) {
        let Some(map) = self.selected_map_name() else {
            self.dialog = Some(Dialog::Error(self.strings().selection_error.to_string()));
            return;
        };
        let target = platform::saves_path(&self.config).join(map);
        let strings = self.strings();

        let Some(zip_path) = rfd::FileDialog::new()
            .set_title(strings.import_select)
            .add_filter("Zip", &["zip"])
            .set_directory(platform::export_dir())
            .pick_file()
        else {
            return; // kullanıcı vazgeçti
        };

        // Önden doğrulanıyor: çakışan ya da aşırı büyük bir arşiv tek bayt
        // yazılmadan reddedilsin.
        match ops::archive_conflicts(&zip_path, &target) {
            Err(error) => {
                self.dialog = Some(Dialog::Error(format!(
                    "{}\n\n{error}",
                    strings.import_error
                )));
                return;
            }
            Ok(conflicts) if !conflicts.is_empty() => {
                self.dialog = Some(Dialog::Error(format!(
                    "{}\n\n{}",
                    strings.import_exists,
                    conflicts.join("\n")
                )));
                return;
            }
            Ok(_) => {}
        }

        log::info!("İçe aktar {} -> {}", zip_path.display(), target.display());
        self.start(ctx, TaskKind::Import, true, move |sink| {
            ops::import_save(&zip_path, &target, ops::MAX_EXTRACT_BYTES, sink)
        });
    }

    // ------------------------------------------------------------------ çizim

    fn header(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            let settings = self.strings().settings;
            if ui.button(RichText::new(settings).size(16.0)).clicked() {
                self.dialog = Some(Dialog::Settings(Box::new(SettingsState::new(&self.config))));
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
            self.selected_save = None;
            self.rescan_saves();
        }
    }

    fn actions(&mut self, ui: &mut Ui, ctx: &Context) {
        let strings = self.strings();
        let has_save = self.selected_save_path().is_some();
        let has_map = self.selected_map_name().is_some();

        ui.add_space(4.0);
        ui.vertical_centered_justified(|ui| {
            if ui
                .add_enabled(has_save, egui::Button::new(strings.backup))
                .clicked()
            {
                self.on_backup(ctx);
            }
            if ui
                .add_enabled(has_save, egui::Button::new(strings.delete))
                .clicked()
            {
                self.on_delete(ctx);
            }
        });

        ui.add_space(4.0);
        ui.columns(2, |columns| {
            if columns[0]
                .add_enabled(has_save, egui::Button::new(strings.export))
                .clicked()
            {
                self.on_export(ctx);
            }
            if columns[1]
                .add_enabled(has_map, egui::Button::new(strings.import))
                .clicked()
            {
                self.on_import(ctx);
            }
        });
    }

    /// İlerleme penceresi — `QProgressDialog` karşılığı.
    fn show_progress(&mut self, ctx: &Context) {
        let Some(handle) = self.task.as_ref() else {
            return;
        };
        let text = self.progress_text(handle.kind());
        let cancel_label = self.strings().cancel;
        let fraction = handle.fraction();
        let (done, total) = handle.progress();
        let cancellable = handle.is_cancellable();

        let mut cancel_clicked = false;
        egui::Modal::new(Id::new("progress")).show(ctx, |ui| {
            ui.set_min_width(320.0);
            ui.label(text);
            ui.add_space(6.0);
            ui.add(egui::ProgressBar::new(fraction).show_percentage());
            if total > 0 {
                ui.label(RichText::new(format!("{done} / {total}")).weak());
            }
            // İptal düğmesi yalnızca iptal edilebilir işlemlerde çizilir.
            // Python bunu `dialog.setCancelButton(None)` ile yapıyor ve ayrıca
            // biten işi iptal etme yarışına karşı sinyali ayırmak zorundaydı;
            // burada iptal bayrağı atomik olduğu için o yarış yok.
            if cancellable {
                ui.add_space(8.0);
                if ui.button(cancel_label).clicked() {
                    cancel_clicked = true;
                }
            }
        });

        if cancel_clicked && let Some(handle) = &self.task {
            handle.cancel();
        }
    }

    /// Bilgi / hata / onay / ayarlar pencerelerini çizer.
    ///
    /// Pencere durumdan `take` edilip yalnızca açık kalması gerekiyorsa geri
    /// konuyor; bu, kapanışı unutmayı imkânsız kılar ve ödünç alma çakışmasını
    /// da ortadan kaldırır.
    fn show_dialog(&mut self, ctx: &Context) {
        let Some(dialog) = self.dialog.take() else {
            return;
        };
        let strings = self.strings();
        let mut keep = None;
        let mut saved = false;

        match dialog {
            Dialog::Info(text) => {
                let mut closed = false;
                egui::Modal::new(Id::new("info")).show(ctx, |ui| {
                    ui.set_min_width(320.0);
                    ui.heading(strings.title);
                    ui.add_space(8.0);
                    ui.label(&text);
                    ui.add_space(10.0);
                    closed = ui.button(strings.close).clicked();
                });
                if !closed {
                    keep = Some(Dialog::Info(text));
                }
            }
            Dialog::Error(text) => {
                let mut closed = false;
                egui::Modal::new(Id::new("error")).show(ctx, |ui| {
                    ui.set_min_width(320.0);
                    ui.heading(RichText::new(strings.error).color(theme::STATUS_TEXT));
                    ui.add_space(8.0);
                    ui.label(&text);
                    ui.add_space(10.0);
                    closed = ui.button(strings.close).clicked();
                });
                if !closed {
                    keep = Some(Dialog::Error(text));
                }
            }
            Dialog::Confirm { text, action } => {
                let mut answer = None;
                egui::Modal::new(Id::new("confirm")).show(ctx, |ui| {
                    ui.set_min_width(320.0);
                    ui.heading(strings.title);
                    ui.add_space(8.0);
                    ui.label(&text);
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        if ui.button(strings.confirm_yes).clicked() {
                            answer = Some(true);
                        }
                        if ui.button(strings.confirm_no).clicked() {
                            answer = Some(false);
                        }
                    });
                });
                match answer {
                    Some(true) => self.run(ctx, action),
                    Some(false) => {}
                    None => keep = Some(Dialog::Confirm { text, action }),
                }
            }
            Dialog::Settings(mut state) => {
                let mut outcome = SettingsOutcome::Open;
                let config = &mut self.config;
                let config_path = &self.config_path;
                egui::Modal::new(Id::new("settings")).show(ctx, |ui| {
                    outcome = state.show(ui, strings, config, config_path);
                });
                match outcome {
                    SettingsOutcome::Open => keep = Some(Dialog::Settings(state)),
                    SettingsOutcome::Saved => saved = true,
                    SettingsOutcome::Dismissed => {}
                }
            }
        }

        self.dialog = keep;
        if saved {
            self.rescan_maps();
        }
    }

    fn run(&mut self, ctx: &Context, action: PendingAction) {
        match action {
            PendingAction::Delete(source) => {
                log::info!("Sil {}", source.display());
                // İptal edilebilir değil: yarıda kesilmiş bir silme, kısmen
                // silinmiş bir save bırakır.
                self.start(ctx, TaskKind::Delete, false, move |sink| {
                    ops::delete_save(&source, sink)
                });
            }
        }
    }
}

impl eframe::App for BackupApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.poll_task();

        // Bir işlem sürerken bütün eylemler kapalı. Python'un
        // `_set_actions_enabled(False)` çağrısının karşılığı, ama tek yerde ve
        // yeni bir düğme eklendiğinde unutulması mümkün değil.
        let idle = self.task.is_none();

        egui::Panel::top("header").show(ui, |ui| {
            ui.add_space(4.0);
            ui.add_enabled_ui(idle, |ui| self.header(ui));
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
            ui.add_enabled_ui(idle, |ui| {
                self.lists(ui);
                self.actions(ui, &ctx);
            });
        });

        self.show_progress(&ctx);
        self.show_dialog(&ctx);
    }
}

/// Çalışan iş parçacığının arayüzü uyandırması için.
fn wake(ctx: &Context) -> task::Wake {
    let ctx = ctx.clone();
    Arc::new(move || ctx.request_repaint())
}

fn index_of(items: &[String], name: &str) -> Option<usize> {
    items.iter().position(|item| item == name)
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

    #[test]
    fn index_of_finds_a_name_or_reports_absence() {
        let items = vec!["a".to_string(), "b".to_string()];
        assert_eq!(index_of(&items, "b"), Some(1));
        assert_eq!(index_of(&items, "c"), None);
    }
}
