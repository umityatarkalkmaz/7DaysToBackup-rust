//! `core::ops` için uçtan uca testler.
//!
//! Python tarafındaki `tests/test_operations.py` dosyasının karşılığı. İki test
//! bilerek Python'un iddiasının tersini doğrular; ilgili yerlerde belirtildi.

use std::cell::Cell;
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use seven_days_to_backup::core::OpError;
use seven_days_to_backup::core::ops::{
    self, DEFAULT_COMPRESSION_LEVEL, NoopSink, PathKind, ProgressSink,
};

// ------------------------------------------------------------------ yardımcılar

/// Save biçiminde küçük bir dizin ağacı oluşturur.
fn build_save(root: &Path, name: &str, files: &[(&str, &[u8])]) -> PathBuf {
    let save_dir = root.join(name);
    for (relative, content) in files {
        let path = save_dir.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, content).unwrap();
    }
    save_dir
}

fn default_save(root: &Path, name: &str) -> PathBuf {
    build_save(
        root,
        name,
        &[
            ("Region/r.0.7rg", b"region-data"),
            ("player.ttp", b"player-data"),
        ],
    )
}

/// Bir ağacı (göreli yol -> içerik) eşlemesine indirger; karşılaştırma için.
fn snapshot(root: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut entries = BTreeMap::new();
    for entry in walkdir::WalkDir::new(root).sort_by_file_name() {
        let entry = entry.unwrap();
        if !entry.file_type().is_file() {
            continue;
        }
        let relative = entry
            .path()
            .strip_prefix(root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        entries.insert(relative, fs::read(entry.path()).unwrap());
    }
    entries
}

fn make_zip(path: &Path, names: &[&str]) -> PathBuf {
    let file = fs::File::create(path).unwrap();
    let mut writer = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default();
    for name in names {
        writer.start_file(*name, options).unwrap();
        writer.write_all(b"data").unwrap();
    }
    writer.finish().unwrap();
    path.to_path_buf()
}

/// `n` çağrıdan sonra iptal bildiren sink.
struct CancelAfter {
    calls: Cell<u64>,
    limit: u64,
}

impl CancelAfter {
    fn new(limit: u64) -> Self {
        Self {
            calls: Cell::new(0),
            limit,
        }
    }
}

impl ProgressSink for CancelAfter {
    fn tick(&self, _done: u64, _total: u64) -> Result<(), OpError> {
        self.calls.set(self.calls.get() + 1);
        if self.calls.get() > self.limit {
            return Err(OpError::Cancelled);
        }
        Ok(())
    }
}

/// İptal dışında bir hatayla patlayan sink — Python'un `explode()` yardımcısı.
struct Explode;

impl ProgressSink for Explode {
    fn tick(&self, _done: u64, _total: u64) -> Result<(), OpError> {
        Err(OpError::Io {
            path: PathBuf::from("/dev/full"),
            source: std::io::Error::other("disk dolu"),
        })
    }
}

/// Gördüğü bütün (done, total) çiftlerini kaydeder.
#[derive(Default)]
struct Recorder {
    seen: std::cell::RefCell<Vec<(u64, u64)>>,
}

impl ProgressSink for Recorder {
    fn tick(&self, done: u64, total: u64) -> Result<(), OpError> {
        self.seen.borrow_mut().push((done, total));
        Ok(())
    }
}

fn temp() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

// ------------------------------------------------------------- kopyala / yedekle

#[test]
fn copy_save_reproduces_the_tree() {
    let dir = temp();
    let source = default_save(dir.path(), "SaveA");
    let destination = dir.path().join("SaveA_backup");

    ops::copy_save(&source, &destination, &NoopSink).unwrap();
    assert_eq!(snapshot(&source), snapshot(&destination));
}

#[test]
fn copy_save_reports_progress_for_every_file() {
    let dir = temp();
    let source = default_save(dir.path(), "SaveA");
    let recorder = Recorder::default();

    ops::copy_save(&source, &dir.path().join("b"), &recorder).unwrap();
    assert_eq!(*recorder.seen.borrow(), vec![(1, 2), (2, 2)]);
}

#[test]
fn copy_save_preserves_empty_directories() {
    let dir = temp();
    let source = default_save(dir.path(), "SaveA");
    fs::create_dir(source.join("EmptyDir")).unwrap();
    let destination = dir.path().join("copy");

    ops::copy_save(&source, &destination, &NoopSink).unwrap();
    assert!(destination.join("EmptyDir").is_dir());
}

#[test]
fn copy_save_carries_the_modification_time() {
    // `shutil.copy2` paritesi: `fs::copy` tek başına mtime taşımaz.
    let dir = temp();
    let source = default_save(dir.path(), "SaveA");
    let old = filetime::FileTime::from_unix_time(1_000_000_000, 0);
    filetime::set_file_mtime(source.join("player.ttp"), old).unwrap();

    let destination = dir.path().join("copy");
    ops::copy_save(&source, &destination, &NoopSink).unwrap();

    let copied = fs::metadata(destination.join("player.ttp")).unwrap();
    assert_eq!(
        filetime::FileTime::from_last_modification_time(&copied),
        old
    );
}

#[test]
fn cancelled_copy_leaves_no_partial_directory() {
    // Yarım bir yedek hayatta kalmamalı: gerçek bir save gibi listelenir.
    let dir = temp();
    let files: Vec<(String, Vec<u8>)> = (0..10)
        .map(|index| (format!("f{index}.bin"), vec![b'x'; 100]))
        .collect();
    let borrowed: Vec<(&str, &[u8])> = files
        .iter()
        .map(|(name, data)| (name.as_str(), data.as_slice()))
        .collect();
    let source = build_save(dir.path(), "SaveA", &borrowed);
    let destination = dir.path().join("partial");

    let error = ops::copy_save(&source, &destination, &CancelAfter::new(3)).unwrap_err();
    assert!(matches!(error, OpError::Cancelled));
    assert!(!destination.exists());
}

#[test]
fn failed_copy_leaves_no_partial_directory() {
    let dir = temp();
    let source = default_save(dir.path(), "SaveA");
    let destination = dir.path().join("partial");

    let error = ops::copy_save(&source, &destination, &Explode).unwrap_err();
    assert!(matches!(error, OpError::Io { .. }));
    assert!(!destination.exists());
}

// ------------------------------------------------------------------ adlandırma

#[test]
fn unique_path_disambiguates_directories() {
    let dir = temp();
    let taken = dir.path().join("SaveA_backup");
    fs::create_dir(&taken).unwrap();

    assert_eq!(
        ops::unique_path(&taken, PathKind::Dir),
        dir.path().join("SaveA_backup_2")
    );
}

#[test]
fn unique_path_keeps_the_extension_for_files() {
    let dir = temp();
    let taken = dir.path().join("SaveA.zip");
    fs::write(&taken, b"").unwrap();

    assert_eq!(
        ops::unique_path(&taken, PathKind::File),
        dir.path().join("SaveA_2.zip")
    );
}

#[test]
fn unique_path_passes_through_when_free() {
    let dir = temp();
    let free = dir.path().join("nothing-here");
    assert_eq!(ops::unique_path(&free, PathKind::Dir), free);
}

#[test]
fn unique_path_does_not_split_a_dotted_directory_name() {
    // Python'dan bilinçli sapma. `os.path.splitext` son noktadan böldüğü için
    // orada bu ad `SaveA_backup_2026.08.16-14.30_2.00` oluyordu.
    let dir = temp();
    let taken = dir.path().join("SaveA_backup_2026.08.16-14.30.00");
    fs::create_dir(&taken).unwrap();

    assert_eq!(
        ops::unique_path(&taken, PathKind::Dir),
        dir.path().join("SaveA_backup_2026.08.16-14.30.00_2")
    );
}

#[test]
fn unique_path_treats_a_leading_dot_as_part_of_the_name() {
    let dir = temp();
    let taken = dir.path().join(".bashrc");
    fs::write(&taken, b"").unwrap();

    assert_eq!(
        ops::unique_path(&taken, PathKind::File),
        dir.path().join(".bashrc_2")
    );
}

#[test]
fn unique_path_keeps_counting_past_the_second_collision() {
    let dir = temp();
    let taken = dir.path().join("SaveA");
    fs::create_dir(&taken).unwrap();
    fs::create_dir(dir.path().join("SaveA_2")).unwrap();
    fs::create_dir(dir.path().join("SaveA_3")).unwrap();

    assert_eq!(
        ops::unique_path(&taken, PathKind::Dir),
        dir.path().join("SaveA_4")
    );
}

#[test]
fn two_backups_in_the_same_second_do_not_collide() {
    let dir = temp();
    let source = default_save(dir.path(), "SaveA");

    let first = ops::unique_path(
        format!(
            "{}_backup_{}",
            source.to_string_lossy(),
            ops::timestamp_suffix()
        ),
        PathKind::Dir,
    );
    ops::copy_save(&source, &first, &NoopSink).unwrap();

    let second = ops::unique_path(
        format!(
            "{}_backup_{}",
            source.to_string_lossy(),
            ops::timestamp_suffix()
        ),
        PathKind::Dir,
    );
    ops::copy_save(&source, &second, &NoopSink).unwrap();

    assert_ne!(first, second);
    assert!(first.is_dir() && second.is_dir());
}

#[test]
fn timestamp_suffix_has_the_documented_shape() {
    let stamp = ops::timestamp_suffix();
    // "2026.08.16-14.30.00"
    assert_eq!(stamp.len(), 19, "{stamp}");
    assert_eq!(stamp.matches('.').count(), 4, "{stamp}");
    assert_eq!(stamp.matches('-').count(), 1, "{stamp}");
}

// ------------------------------------------------------------ dışa / içe aktar

#[test]
fn export_import_round_trip() {
    let dir = temp();
    let source = default_save(dir.path(), "SaveA");
    let zip_path = dir.path().join("out.zip");
    ops::export_save(&source, &zip_path, DEFAULT_COMPRESSION_LEVEL, &NoopSink).unwrap();

    let target = dir.path().join("target");
    fs::create_dir(&target).unwrap();
    ops::import_save(&zip_path, &target, ops::MAX_EXTRACT_BYTES, &NoopSink).unwrap();

    assert_eq!(snapshot(&source), snapshot(&target.join("SaveA")));
}

#[test]
fn round_trip_is_identical_at_every_compression_level() {
    for level in [1, 6, 9] {
        let dir = temp();
        let source = default_save(dir.path(), "SaveA");
        let zip_path = dir.path().join(format!("out{level}.zip"));
        ops::export_save(&source, &zip_path, level, &NoopSink).unwrap();

        let target = dir.path().join(format!("t{level}"));
        fs::create_dir(&target).unwrap();
        ops::import_save(&zip_path, &target, ops::MAX_EXTRACT_BYTES, &NoopSink).unwrap();

        assert_eq!(
            snapshot(&source),
            snapshot(&target.join("SaveA")),
            "seviye {level}"
        );
    }
}

#[test]
fn exported_entries_use_forward_slashes() {
    // Windows'ta ters bölü ile yazılan bir arşiv, Linux/macOS'ta tek bir uzun
    // dosya adı gibi açılır.
    let dir = temp();
    let source = default_save(dir.path(), "SaveA");
    let zip_path = dir.path().join("out.zip");
    ops::export_save(&source, &zip_path, DEFAULT_COMPRESSION_LEVEL, &NoopSink).unwrap();

    let file = fs::File::open(&zip_path).unwrap();
    let archive = zip::ZipArchive::new(file).unwrap();
    let names: Vec<_> = archive.file_names().collect();

    assert!(names.iter().all(|name| !name.contains('\\')), "{names:?}");
    assert!(
        names.contains(&"SaveA/Region/r.0.7rg"),
        "arşiv kökünde save adı yok: {names:?}"
    );
}

#[test]
fn export_does_not_clobber_a_previous_archive() {
    let dir = temp();
    let source = default_save(dir.path(), "SaveA");

    let first = ops::unique_path(dir.path().join("SaveA.zip"), PathKind::File);
    ops::export_save(&source, &first, DEFAULT_COMPRESSION_LEVEL, &NoopSink).unwrap();
    let second = ops::unique_path(dir.path().join("SaveA.zip"), PathKind::File);
    ops::export_save(&source, &second, DEFAULT_COMPRESSION_LEVEL, &NoopSink).unwrap();

    assert_ne!(first, second);
    assert!(first.exists() && second.exists());
}

#[test]
fn cancelled_export_removes_the_partial_archive() {
    let dir = temp();
    let files: Vec<(String, Vec<u8>)> = (0..10)
        .map(|index| (format!("f{index}.bin"), vec![b'x'; 50]))
        .collect();
    let borrowed: Vec<(&str, &[u8])> = files
        .iter()
        .map(|(name, data)| (name.as_str(), data.as_slice()))
        .collect();
    let source = build_save(dir.path(), "SaveA", &borrowed);
    let zip_path = dir.path().join("partial.zip");

    let error = ops::export_save(
        &source,
        &zip_path,
        DEFAULT_COMPRESSION_LEVEL,
        &CancelAfter::new(2),
    )
    .unwrap_err();

    assert!(matches!(error, OpError::Cancelled));
    assert!(
        !zip_path.exists(),
        "yarım arşiv kullanılabilir bir yedek değil"
    );
}

#[test]
fn archive_uncompressed_size_sums_every_entry() {
    let dir = temp();
    let zip_path = make_zip(&dir.path().join("a.zip"), &["a.txt", "b.txt", "c.txt"]);
    // Her girdi "data" yazıyor: 3 x 4 bayt.
    assert_eq!(ops::archive_uncompressed_size(&zip_path).unwrap(), 12);
}

// ------------------------------------------------------------- içe aktarma kalkanları

#[test]
fn conflicts_detects_a_second_root() {
    // Python'un ilk hali yalnızca members[0]'a bakıyordu, SaveB kaçıyordu.
    let dir = temp();
    let zip_path = make_zip(
        &dir.path().join("multi.zip"),
        &["SaveA/file.txt", "SaveB/file.txt"],
    );
    let target = dir.path().join("maps");
    fs::create_dir_all(target.join("SaveB")).unwrap();

    assert_eq!(
        ops::archive_conflicts(&zip_path, &target).unwrap(),
        vec!["SaveB".to_string()]
    );
}

#[test]
fn conflicts_detects_a_folder_when_the_first_entry_is_a_loose_file() {
    let dir = temp();
    let zip_path = make_zip(
        &dir.path().join("filefirst.zip"),
        &["readme.txt", "SaveA/file.txt"],
    );
    let target = dir.path().join("maps");
    fs::create_dir_all(target.join("SaveA")).unwrap();

    assert_eq!(
        ops::archive_conflicts(&zip_path, &target).unwrap(),
        vec!["SaveA".to_string()]
    );
}

#[test]
fn no_conflicts_when_the_target_is_clean() {
    let dir = temp();
    let zip_path = make_zip(&dir.path().join("clean.zip"), &["SaveA/file.txt"]);
    let target = dir.path().join("maps");
    fs::create_dir_all(&target).unwrap();

    assert!(
        ops::archive_conflicts(&zip_path, &target)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn import_refuses_an_oversized_archive() {
    let dir = temp();
    let zip_path = make_zip(&dir.path().join("big.zip"), &["SaveA/file.txt"]);
    let target = dir.path().join("maps");
    fs::create_dir_all(&target).unwrap();

    let error = ops::import_save(&zip_path, &target, 1, &NoopSink).unwrap_err();
    assert!(matches!(error, OpError::TooLarge { .. }), "{error}");
    assert_eq!(fs::read_dir(&target).unwrap().count(), 0);
}

#[test]
fn import_refuses_an_empty_archive() {
    let dir = temp();
    let zip_path = make_zip(&dir.path().join("empty.zip"), &[]);
    let target = dir.path().join("maps");
    fs::create_dir_all(&target).unwrap();

    let error =
        ops::import_save(&zip_path, &target, ops::MAX_EXTRACT_BYTES, &NoopSink).unwrap_err();
    assert!(matches!(error, OpError::EmptyArchive), "{error}");
}

#[test]
fn import_rejects_an_archive_that_escapes_the_target() {
    // **Python'un tersini iddia eder, bilinçli.** CPython'un zipfile'ı '..'
    // bileşenlerini sessizce kırpıyor ve dosyayı hedefin içine düşürüyordu;
    // Python testi de bunu doğruluyor. Burada arşiv reddediliyor.
    let dir = temp();
    let zip_path = make_zip(&dir.path().join("evil.zip"), &["../../escaped.txt"]);
    let target = dir.path().join("maps");
    fs::create_dir_all(&target).unwrap();

    let error =
        ops::import_save(&zip_path, &target, ops::MAX_EXTRACT_BYTES, &NoopSink).unwrap_err();
    assert!(matches!(error, OpError::UnsafePath(_)), "{error}");
    assert!(!dir.path().join("escaped.txt").exists());
    assert!(!target.join("escaped.txt").exists());
}

#[test]
fn cancelled_import_rolls_back_what_it_created() {
    // Python yarım açılmış dosyaları bırakıyordu. Geri alma güvenli, çünkü içe
    // aktarma yalnızca archive_conflicts temiz döndükten sonra başlar.
    let dir = temp();
    let names: Vec<String> = (0..10).map(|index| format!("SaveA/f{index}.txt")).collect();
    let borrowed: Vec<&str> = names.iter().map(String::as_str).collect();
    let zip_path = make_zip(&dir.path().join("many.zip"), &borrowed);
    let target = dir.path().join("maps");
    fs::create_dir_all(&target).unwrap();

    let error = ops::import_save(
        &zip_path,
        &target,
        ops::MAX_EXTRACT_BYTES,
        &CancelAfter::new(3),
    )
    .unwrap_err();

    assert!(matches!(error, OpError::Cancelled));
    assert!(!target.join("SaveA").exists(), "geri alma çalışmadı");
    assert_eq!(fs::read_dir(&target).unwrap().count(), 0);
}

#[test]
fn successful_import_keeps_its_output() {
    // Geri alma nöbetçisinin başarı yolunda tetiklenmediğini doğrular.
    let dir = temp();
    let zip_path = make_zip(&dir.path().join("ok.zip"), &["SaveA/file.txt"]);
    let target = dir.path().join("maps");
    fs::create_dir_all(&target).unwrap();

    ops::import_save(&zip_path, &target, ops::MAX_EXTRACT_BYTES, &NoopSink).unwrap();
    assert!(target.join("SaveA").join("file.txt").is_file());
}

// ------------------------------------------------------------------------ sil

#[test]
fn delete_save_removes_the_tree() {
    let dir = temp();
    let source = default_save(dir.path(), "SaveA");
    ops::delete_save(&source, &NoopSink).unwrap();
    assert!(!source.exists());
}

#[test]
fn delete_save_ignores_cancellation() {
    // İptal edilebilir değil: yarıda kesilmiş bir silme, tamamlanmamış silmeden
    // daha kötüdür.
    let dir = temp();
    let source = default_save(dir.path(), "SaveA");
    ops::delete_save(&source, &CancelAfter::new(0)).unwrap();
    assert!(!source.exists());
}

#[test]
fn delete_save_reports_a_missing_directory() {
    let dir = temp();
    let error = ops::delete_save(&dir.path().join("not-here"), &NoopSink).unwrap_err();
    assert!(matches!(error, OpError::Io { .. }), "{error}");
}
