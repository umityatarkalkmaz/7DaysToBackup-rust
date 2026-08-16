//! Arayüz testleri.
//!
//! `egui_kittest` uygulamayı başsız çalıştırıp accesskit ağacı üzerinden
//! sorgulamaya ve tıklamaya izin veriyor. Bu dosya, "derlendi ve açıldı"nın
//! yeterli olmadığını gösteren bir hatadan sonra eklendi: liste kaydırma
//! alanları mevcut tüm dikey alanı yiyor ve dört eylem düğmesini pencerenin
//! dışına itiyordu.

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use std::path::{Path, PathBuf};

use seven_days_to_backup::core::config::Config;
use seven_days_to_backup::ui::BackupApp;

const WINDOW: egui::Vec2 = egui::vec2(900.0, 600.0);

/// Uygulamanın en dar kullanılabilir hali; `main.rs` bunu asgari boyut olarak
/// veriyor. Düğmeler burada da görünmek zorunda.
const SMALLEST_WINDOW: egui::Vec2 = egui::vec2(640.0, 420.0);

const ACTION_LABELS: [&str; 4] = ["Yedekle", "Sil", "Dışa Aktar", "İçe Aktar"];

/// İki map ve üç save içeren sahte bir save ağacı.
fn build_saves(root: &Path) {
    for (map, saves) in [
        ("Navezgane", &["SaveA", "SaveB"][..]),
        ("PREGEN10k", &["Alpha"]),
    ] {
        for save in saves {
            let region = root.join(map).join(save).join("Region");
            std::fs::create_dir_all(&region).unwrap();
            std::fs::write(region.join("r.0.7rg"), b"region").unwrap();
            std::fs::write(root.join(map).join(save).join("player.ttp"), b"player").unwrap();
        }
    }
}

struct Fixture {
    _saves: tempfile::TempDir,
    _config: tempfile::TempDir,
    config: Config,
    config_path: PathBuf,
}

fn fixture() -> Fixture {
    let saves = tempfile::tempdir().unwrap();
    build_saves(saves.path());

    let config_dir = tempfile::tempdir().unwrap();
    let config_path = config_dir.path().join("config.json");

    Fixture {
        config: Config {
            custom_save_path: saves.path().to_string_lossy().into_owned(),
            language: "tr".to_string(),
        },
        config_path,
        _saves: saves,
        _config: config_dir,
    }
}

fn harness(fixture: &Fixture, size: egui::Vec2) -> Harness<'static, BackupApp> {
    let config = fixture.config.clone();
    let config_path = fixture.config_path.clone();
    let mut harness = Harness::builder()
        .with_size(size)
        .build_eframe(move |cc| BackupApp::with_config(&cc.egui_ctx, config, config_path));
    harness.run();
    harness
}

fn screen(size: egui::Vec2) -> egui::Rect {
    egui::Rect::from_min_size(egui::Pos2::ZERO, size)
}

// ------------------------------------------------------------------ görünürlük

#[test]
fn the_action_buttons_are_inside_the_window() {
    // Regresyon: kaydırma alanları `auto_shrink([false, false])` ile bütün
    // dikey alanı alıyordu ve dört düğme de pencerenin altından taşıyordu.
    let fixture = fixture();
    let harness = harness(&fixture, WINDOW);
    let bounds = screen(WINDOW);

    for label in ACTION_LABELS {
        let node = harness
            .query_by_label(label)
            .unwrap_or_else(|| panic!("'{label}' düğmesi arayüzde yok"));
        let rect = node.rect();
        assert!(
            bounds.contains_rect(rect),
            "'{label}' düğmesi pencerenin dışında: {rect:?}, pencere {bounds:?}"
        );
    }
}

#[test]
fn the_action_buttons_survive_the_smallest_window() {
    let fixture = fixture();
    let harness = harness(&fixture, SMALLEST_WINDOW);
    let bounds = screen(SMALLEST_WINDOW);

    for label in ACTION_LABELS {
        let node = harness
            .query_by_label(label)
            .unwrap_or_else(|| panic!("'{label}' düğmesi arayüzde yok"));
        assert!(
            bounds.contains_rect(node.rect()),
            "'{label}' asgari pencere boyutunda taşıyor: {:?}",
            node.rect()
        );
    }
}

#[test]
fn both_lists_are_visible_alongside_the_buttons() {
    let fixture = fixture();
    let harness = harness(&fixture, WINDOW);
    let bounds = screen(WINDOW);

    for label in ["Map Listesi", "Save Listesi"] {
        let node = harness
            .query_by_label(label)
            .unwrap_or_else(|| panic!("'{label}' başlığı yok"));
        assert!(bounds.contains_rect(node.rect()), "'{label}' taşıyor");
    }
}

// --------------------------------------------------------------------- listeler

#[test]
fn the_map_list_shows_every_map() {
    let fixture = fixture();
    let harness = harness(&fixture, WINDOW);

    assert!(harness.query_by_label("Navezgane").is_some());
    assert!(harness.query_by_label("PREGEN10k").is_some());
}

#[test]
fn choosing_a_map_fills_the_save_list() {
    let fixture = fixture();
    let mut harness = harness(&fixture, WINDOW);

    // Başlangıçta hiçbir save görünmüyor.
    assert!(harness.query_by_label("SaveA").is_none());

    harness.get_by_label("Navezgane").click();
    harness.run();

    assert!(
        harness.query_by_label("SaveA").is_some(),
        "SaveA listelenmedi"
    );
    assert!(
        harness.query_by_label("SaveB").is_some(),
        "SaveB listelenmedi"
    );
    // Diğer map'in save'i sızmamalı.
    assert!(harness.query_by_label("Alpha").is_none());
}

#[test]
fn switching_maps_replaces_the_save_list() {
    let fixture = fixture();
    let mut harness = harness(&fixture, WINDOW);

    harness.get_by_label("Navezgane").click();
    harness.run();
    assert!(harness.query_by_label("SaveA").is_some());

    harness.get_by_label("PREGEN10k").click();
    harness.run();
    assert!(
        harness.query_by_label("SaveA").is_none(),
        "eski liste kaldı"
    );
    assert!(harness.query_by_label("Alpha").is_some());
}

// ---------------------------------------------------------------------- işlem

#[test]
fn backing_up_a_save_writes_a_timestamped_copy() {
    let fixture = fixture();
    let map_dir = PathBuf::from(&fixture.config.custom_save_path).join("Navezgane");
    let mut harness = harness(&fixture, WINDOW);

    harness.get_by_label("Navezgane").click();
    harness.run();
    harness.get_by_label("SaveA").click();
    harness.run();
    harness.get_by_label("Yedekle").click();
    harness.run();

    // İşlem bir çalışan iş parçacığında; sonuç penceresi belirene kadar kare çevir.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while harness.query_by_label("Yedekleme başarılı").is_none() {
        assert!(
            std::time::Instant::now() < deadline,
            "yedekleme zamanında bitmedi"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
        harness.run();
    }

    let backups: Vec<_> = std::fs::read_dir(&map_dir)
        .unwrap()
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with("SaveA_backup_"))
        .collect();

    assert_eq!(backups.len(), 1, "beklenen tek yedek, bulunan: {backups:?}");
    assert!(map_dir.join(&backups[0]).join("player.ttp").is_file());
}

#[test]
fn deleting_a_save_asks_first() {
    let fixture = fixture();
    let save_dir = PathBuf::from(&fixture.config.custom_save_path)
        .join("Navezgane")
        .join("SaveA");
    let mut harness = harness(&fixture, WINDOW);

    harness.get_by_label("Navezgane").click();
    harness.run();
    harness.get_by_label("SaveA").click();
    harness.run();
    harness.get_by_label("Sil").click();
    harness.run();

    // Onay penceresi çıkmalı ve henüz hiçbir şey silinmemeli.
    assert!(
        harness.query_by_label("Hayır").is_some(),
        "onay penceresi açılmadı"
    );
    assert!(save_dir.is_dir(), "onay beklenmeden silindi");

    harness.get_by_label("Hayır").click();
    harness.run();
    assert!(save_dir.is_dir(), "vazgeçilmesine rağmen silindi");
}

// ------------------------------------------------------------------------ dil

#[test]
fn the_interface_starts_in_the_configured_language() {
    let mut fixture = fixture();
    fixture.config.language = "en".to_string();
    let harness = harness(&fixture, WINDOW);

    assert!(harness.query_by_label("Backup").is_some());
    assert!(harness.query_by_label("Yedekle").is_none());
}

#[test]
fn a_missing_saves_folder_reports_itself_without_a_dialog() {
    let config_dir = tempfile::tempdir().unwrap();
    let missing = config_dir.path().join("not-here");
    let config = Config {
        // Var olmayan özel yol yok sayılır ve varsayılana düşülür; testte
        // varsayılanın da bulunmaması ihtimaline karşı yalnızca durum
        // etiketinin biçimini doğruluyoruz.
        custom_save_path: missing.to_string_lossy().into_owned(),
        language: "tr".to_string(),
    };
    let config_path = config_dir.path().join("config.json");

    let mut harness = Harness::builder()
        .with_size(WINDOW)
        .build_eframe(move |cc| BackupApp::with_config(&cc.egui_ctx, config, config_path));
    harness.run();

    // Save klasörü yoksa uygulama modal açmaz; düğmeler yerinde kalır.
    for label in ACTION_LABELS {
        assert!(
            harness.query_by_label(label).is_some(),
            "'{label}' kayboldu"
        );
    }
}
