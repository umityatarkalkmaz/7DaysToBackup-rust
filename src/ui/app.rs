use eframe::CreationContext;
use egui::{RichText, ThemePreference, Ui};

/// Uygulamanın kök durumu. Save tarama, yedekleme ve dışa aktarma alanları
/// buraya eklenecek; şimdilik yalnızca pencere iskeleti var.
pub struct BackupApp {}

impl BackupApp {
    pub fn new(cc: &CreationContext<'_>) -> Self {
        // Python sürümü koyu tema ile geliyor, aynı varsayılanı koruyoruz.
        cc.egui_ctx.set_theme(ThemePreference::Dark);
        Self {}
    }
}

impl eframe::App for BackupApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        egui::Panel::top("header").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading("7 Days To Backup");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new(env!("CARGO_PKG_VERSION")).weak());
                });
            });
        });

        egui::CentralPanel::default().show(ui, |ui| {
            ui.label("Save tarama ve yedekleme arayüzü buraya gelecek.");
        });
    }
}
