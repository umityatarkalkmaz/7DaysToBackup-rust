// Konsol penceresi Windows'ta release build'de açılmasın.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use seven_days_to_backup::core::log;
use seven_days_to_backup::ui::BackupApp;

fn main() -> eframe::Result {
    // Tutamaç uygulamanın ömrü boyunca canlı kalmalı: düştüğünde flexi_logger
    // arabelleği boşaltıp dosyayı kapatır, sonraki kayıtlar diske ulaşmaz.
    let _logger = log::init();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([900.0, 600.0])
            .with_min_inner_size([640.0, 420.0])
            .with_title("7 Days To Backup"),
        ..Default::default()
    };

    eframe::run_native(
        "7DaysToBackup",
        options,
        Box::new(|cc| Ok(Box::new(BackupApp::new(cc)))),
    )
}
