//! 7DaysToBackup — 7 Days to Die save yedekleme aracı.
//!
//! Katman kuralı: `core` ve `task` içinde `egui`/`eframe` importu yoktur. Python
//! sürümünde `core/` paketinin Qt'siz tutulmasıyla aynı gerekçe — bu modüllerin
//! testleri ekransız bir makinede de koşabilmeli.

pub mod core;
pub mod i18n;
pub mod task;
pub mod ui;
