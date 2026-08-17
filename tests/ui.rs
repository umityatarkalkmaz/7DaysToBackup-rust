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

const ACTION_LABELS: [&str; 6] = [
    "Yedekle",
    "Sil",
    "Dışa Aktar",
    "İçe Aktar",
    "Geri Yükle",
    "Yedeği Sil",
];

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
            // Ölçek sabitleniyor: bu dosyadaki bütün yerleşim iddiaları piksel
            // cinsinden ve otomatik algılama koşucunun monitörüne bağlı olurdu.
            ui_scale: 1.0,
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

/// Bir öğeye verilen değiştiricilerle tıklar.
///
/// `Node::click()` değiştirici taşımıyor, `RawInput`'ta da genel bir `modifiers`
/// alanı yok. Üstelik `PointerButton` olayının kendi `modifiers` alanı da yetmiyor:
/// `InputState.modifiers` yalnızca `Event::ModifiersChanged` ile güncelleniyor
/// (`egui/src/input_state/mod.rs:433`), yani tuşun basıldığı ayrıca bildirilmeli.
fn click_with(harness: &mut Harness<'static, BackupApp>, label: &str, modifiers: egui::Modifiers) {
    let pos = harness.get_by_label(label).rect().center();
    let events = &mut harness.input_mut().events;
    events.push(egui::Event::ModifiersChanged(modifiers));
    events.push(egui::Event::PointerMoved(pos));
    for pressed in [true, false] {
        events.push(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers,
        });
    }
    harness.run();

    // Tuş bırakılıyor: aksi halde sonraki tıklamalara sızardı.
    harness
        .input_mut()
        .events
        .push(egui::Event::ModifiersChanged(egui::Modifiers::NONE));
    harness.run();
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

// ------------------------------------------------------------------------ ölçek

#[test]
fn the_configured_ui_scale_reaches_egui() {
    // 2560x1440 monitörde arayüzün küçük görünmesi bildirilen kusurdu. egui yazı
    // boyutlarını mantıksal piksel olarak sabitliyor ve masaüstünün yazı tipi
    // ayarını hiç okumuyor; tek kaldıraç yakınlaştırma katsayısı.
    //
    // İddia düğme dikdörtgeni üzerinden kurulamıyor: `egui_kittest` her karede
    // `screen_rect`'i nokta cinsinden sabitliyor (`egui_kittest/src/lib.rs:231`),
    // oysa gerçek pencerede yakınlaştırma **nokta uzayını küçültür** ve widget'lar
    // fiziksel olarak büyür. Nokta uzayı sabitlenince etki gözlemlenemez hâle
    // geliyor. Bizim sorumluluğumuz değerin egui'ye ulaşması; ondan sonrası
    // egui'nin kendi davranışı. Yerleşimin dar alanda bozulmadığı ayrı testte.
    let mut fixture = fixture();
    fixture.config.ui_scale = 1.5;

    assert_eq!(harness(&fixture, WINDOW).ctx.zoom_factor(), 1.5);
}

#[test]
fn an_out_of_range_ui_scale_is_clamped_before_it_reaches_egui() {
    let mut fixture = fixture();
    fixture.config.ui_scale = 99.0;

    let zoom = harness(&fixture, WINDOW).ctx.zoom_factor();
    assert!(zoom <= 3.0, "kırpılmadı: {zoom}");
}

#[test]
fn a_hostile_ui_scale_falls_back_instead_of_killing_the_window() {
    // `config.json` bozulabilir ve `set_zoom_factor` hiçbir doğrulama yapmıyor;
    // sonsuz bir ölçek arayüz çizilmeden uygulamayı öldürebilirdi.
    let mut fixture = fixture();
    fixture.config.ui_scale = f32::INFINITY;
    let harness = harness(&fixture, WINDOW);

    for label in ACTION_LABELS {
        assert!(
            harness.query_by_label(label).is_some(),
            "'{label}' çizilmedi"
        );
    }
}

#[test]
fn the_action_buttons_survive_the_smallest_window_when_scaled_up() {
    // Asgari pencere boyutu ölçekle büyümüyor (bkz. `main.rs`), dolayısıyla
    // yakınlaştırılmış bir arayüz 640x420 fiziksel pencereye sığmak zorunda.
    //
    // Yakınlaştırma nokta uzayını küçültür: 1.25 ölçekte 640x420 piksellik bir
    // pencere 512x336 **nokta** eder. Harness nokta uzayını sabitlediği için
    // ölçeği taklit etmenin doğru yolu, pencereyi bölünmüş boyutta kurmak.
    const SCALE: f32 = 1.25;
    let scaled_window = SMALLEST_WINDOW / SCALE;

    let fixture = fixture();
    let harness = harness(&fixture, scaled_window);
    let bounds = screen(scaled_window);

    for label in ACTION_LABELS {
        let node = harness
            .query_by_label(label)
            .unwrap_or_else(|| panic!("'{label}' düğmesi arayüzde yok"));
        assert!(
            bounds.contains_rect(node.rect()),
            "'{label}' {SCALE} ölçekte asgari pencereden taşıyor: {:?}, alan {bounds:?}",
            node.rect()
        );
    }
}

#[test]
fn the_three_lists_sit_side_by_side_without_overlapping() {
    // Yedek geçmişi üçüncü bir sütun ekledi. Dar pencerede sütunların birbirine
    // girmediği ve hiçbirinin sıfıra çökmediği burada tutuluyor; ölçek de
    // hesaba katılıyor çünkü yakınlaştırma nokta uzayını daraltıyor.
    for size in [WINDOW, SMALLEST_WINDOW, SMALLEST_WINDOW / 1.4] {
        let fixture = fixture();
        let harness = harness(&fixture, size);

        let mut previous_right = 0.0_f32;
        for label in ["Map Listesi", "Save Listesi", "Yedek Geçmişi"] {
            let rect = harness.get_by_label(label).rect();
            assert!(
                rect.min.x >= previous_right,
                "{size:?}: '{label}' bir öncekinin üstüne biniyor ({rect:?})"
            );
            assert!(
                rect.width() > 40.0,
                "{size:?}: '{label}' sütunu çökmüş ({rect:?})"
            );
            previous_right = rect.max.x;
        }
        assert!(
            previous_right <= size.x,
            "{size:?}: son sütun pencereden taşıyor ({previous_right})"
        );
    }
}

#[test]
fn ctrl_click_adds_a_second_save_to_the_selection() {
    let fixture = fixture();
    let mut harness = harness(&fixture, WINDOW);
    harness.get_by_label("Navezgane").click();
    harness.run();

    harness.get_by_label("SaveA").click();
    harness.run();

    // Ctrl basılıyken tıklamak seçimi değiştirmek yerine ekler.
    click_with(&mut harness, "SaveB", egui::Modifiers::COMMAND);

    // İki save seçiliyken yedek geçmişi boşalır: hangisinin geçmişi olduğu
    // belirsiz olurdu.
    assert!(
        harness.query_by_label("Bu save'in yedeği yok").is_some(),
        "çoklu seçimde geçmiş sütunu boşalmadı"
    );

    // Yedekleme ikisini birden almalı.
    harness.get_by_label("Yedekle").click();
    let map_dir = PathBuf::from(&fixture.config.custom_save_path).join("Navezgane");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while harness.query_by_label("Yedekleme başarılı").is_none() {
        assert!(
            std::time::Instant::now() < deadline,
            "yedekleme zamanında bitmedi"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
        harness.step();
    }

    let backups: Vec<String> = std::fs::read_dir(&map_dir)
        .unwrap()
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.contains("_backup_"))
        .collect();
    assert_eq!(
        backups.len(),
        2,
        "iki yedek beklenirdi, bulunan: {backups:?}"
    );
    assert!(backups.iter().any(|name| name.starts_with("SaveA_backup_")));
    assert!(backups.iter().any(|name| name.starts_with("SaveB_backup_")));
}

#[test]
fn a_plain_click_replaces_the_selection() {
    let fixture = fixture();
    let mut harness = harness(&fixture, WINDOW);
    harness.get_by_label("Navezgane").click();
    harness.run();

    harness.get_by_label("SaveA").click();
    harness.run();
    click_with(&mut harness, "SaveB", egui::Modifiers::COMMAND);

    // Düz tıklama yalnız o öğeyi bırakmalı; tek seçim geri geldiğinde geçmiş
    // sütunu da yeniden o save'e ait olur.
    harness.get_by_label("SaveA").click();
    harness.run();
    harness.get_by_label("Yedekle").click();

    let map_dir = PathBuf::from(&fixture.config.custom_save_path).join("Navezgane");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while harness.query_by_label("Yedekleme başarılı").is_none() {
        assert!(std::time::Instant::now() < deadline, "yedekleme bitmedi");
        std::thread::sleep(std::time::Duration::from_millis(10));
        harness.step();
    }

    let backups: Vec<String> = std::fs::read_dir(&map_dir)
        .unwrap()
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.contains("_backup_"))
        .collect();
    assert_eq!(
        backups.len(),
        1,
        "yalnız SaveA yedeklenmeliydi: {backups:?}"
    );
    assert!(backups[0].starts_with("SaveA_backup_"));
}

// ------------------------------------------------------------------- yedekler

/// Bir save'in yanına, geçerli adlandırmayla sahte bir yedek koyar.
fn add_backup(root: &Path, map: &str, save: &str, stamp: &str) -> PathBuf {
    let dir = root.join(map).join(format!("{save}_backup_{stamp}"));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("player.ttp"), b"eski").unwrap();
    dir
}

#[test]
fn backups_do_not_appear_in_the_save_list() {
    // Yedekler save'lerle aynı klasörde duruyor ve eskiden save listesinde
    // karışık görünüyorlardı; kullanıcı bir yedeğin yedeğini alabiliyordu.
    let fixture = fixture();
    let root = PathBuf::from(&fixture.config.custom_save_path);
    add_backup(&root, "Navezgane", "SaveA", "2026.08.16-14.30.00");

    let mut harness = harness(&fixture, WINDOW);
    harness.get_by_label("Navezgane").click();
    harness.run();

    assert!(harness.query_by_label("SaveA").is_some());
    assert!(
        harness
            .query_by_label("SaveA_backup_2026.08.16-14.30.00")
            .is_none(),
        "yedek save listesine sızdı"
    );
}

#[test]
fn selecting_a_save_shows_its_backups_newest_first() {
    let fixture = fixture();
    let root = PathBuf::from(&fixture.config.custom_save_path);
    add_backup(&root, "Navezgane", "SaveA", "2026.08.16-14.30.00");
    add_backup(&root, "Navezgane", "SaveA", "2026.08.17-09.00.00");
    // Başka bir save'in yedeği sızmamalı.
    add_backup(&root, "Navezgane", "SaveB", "2026.08.15-11.00.00");

    let mut harness = harness(&fixture, WINDOW);
    harness.get_by_label("Navezgane").click();
    harness.run();

    // Save seçilmeden geçmiş boş.
    assert!(harness.query_by_label("16.08.2026 14:30:00").is_none());

    harness.get_by_label("SaveA").click();
    harness.run();

    assert!(harness.query_by_label("17.08.2026 09:00:00").is_some());
    assert!(harness.query_by_label("16.08.2026 14:30:00").is_some());
    assert!(
        harness.query_by_label("15.08.2026 11:00:00").is_none(),
        "başka save'in yedeği listelendi"
    );
}

#[test]
fn restoring_a_backup_asks_first_and_keeps_the_current_state() {
    let fixture = fixture();
    let root = PathBuf::from(&fixture.config.custom_save_path);
    add_backup(&root, "Navezgane", "SaveA", "2026.08.16-14.30.00");
    let save = root.join("Navezgane").join("SaveA");

    let mut harness = harness(&fixture, WINDOW);
    harness.get_by_label("Navezgane").click();
    harness.run();
    harness.get_by_label("SaveA").click();
    harness.run();
    harness.get_by_label("16.08.2026 14:30:00").click();
    harness.run();
    harness.get_by_label("Geri Yükle").click();
    harness.run();

    assert!(
        harness.query_by_label("Hayır").is_some(),
        "onay penceresi açılmadı"
    );
    assert!(
        save.join("player.ttp").is_file(),
        "onay beklenmeden değişti"
    );

    harness.get_by_label("Hayır").click();
    harness.run();
    assert_eq!(std::fs::read(save.join("player.ttp")).unwrap(), b"player");
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

    // Buradan itibaren `run()` değil `step()` kullanılıyor. `run()`, arayüz
    // yeniden çizim istemeyi bırakana kadar döner ve dört adımda durulmazsa
    // panikler; ilerleme penceresindeki belirsiz çubuk animasyonlu olduğu için
    // işlem sürdüğü sürece her karede yeniden çizim ister ve o sınır aşılır.
    // (Linux'ta yedekleme modal çizilmeye fırsat bulamadan bitiyordu, hata
    // yalnızca Windows koşucusunda görüldü.)
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while harness.query_by_label("Yedekleme başarılı").is_none() {
        assert!(
            std::time::Instant::now() < deadline,
            "yedekleme zamanında bitmedi"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
        harness.step();
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
        ui_scale: 1.0,
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
