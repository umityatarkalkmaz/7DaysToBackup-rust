// Konsol penceresi Windows'ta release build'de açılmasın.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;

use app::BackupApp;

fn main() -> eframe::Result {
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
