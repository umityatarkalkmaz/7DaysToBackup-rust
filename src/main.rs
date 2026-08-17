// Konsol penceresi Windows'ta release build'de açılmasın.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use seven_days_to_backup::core::config::Config;
use seven_days_to_backup::core::{log, paths};
use seven_days_to_backup::ui::{BackupApp, theme};

/// Ölçek 1.0'da açılış penceresi.
const WINDOW: [f32; 2] = [900.0, 600.0];
/// Kullanıcının küçültebileceği en dar hâl; `tests/ui.rs` düğmelerin burada da
/// göründüğünü doğruluyor. Ölçekle çarpılmıyor: asgari boyut bir taban.
const MIN_WINDOW: [f32; 2] = [640.0, 420.0];

fn main() -> eframe::Result {
    // Tutamaç uygulamanın ömrü boyunca canlı kalmalı: düştüğünde flexi_logger
    // arabelleği boşaltıp dosyayı kapatır, sonraki kayıtlar diske ulaşmaz.
    let _logger = log::init();

    // Yapılandırma burada okunuyor ve `BackupApp`'e devrediliyor: pencere boyutu
    // `Context` kurulmadan önce belirleniyor, dolayısıyla ölçeği oradan
    // öğrenemiyoruz. İki kez okumak yerine bir kez okuyup taşıyoruz.
    let config_path = paths::config_file();
    let config = Config::load(&config_path);
    // Otomatik moddayken 1.0: monitör boyutu da henüz bilinmiyor. `BackupApp`
    // pencere açıldıktan sonra gerçek ölçeği uyguluyor.
    let scale = config.ui_scale().unwrap_or(1.0);

    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([WINDOW[0] * scale, WINDOW[1] * scale])
        .with_min_inner_size(MIN_WINDOW)
        .with_title("7 Days To Backup");
    // İkon kozmetik: çözülemezse pencere ikonsuz açılır, uygulama düşmez.
    if let Some(icon) = theme::icon() {
        viewport = viewport.with_icon(icon);
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "7DaysToBackup",
        options,
        Box::new(move |cc| {
            Ok(Box::new(BackupApp::with_config(
                &cc.egui_ctx,
                config,
                config_path,
            )))
        }),
    )
}
