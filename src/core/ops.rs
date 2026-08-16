//! Save klasörü işlemleri: yedekle, sil, dışa aktar, içe aktar.
//!
//! Bilerek `egui`'den arınmış: hem ekransız test edilebilsinler hem de bir
//! çalışan iş parçacığında widget'a dokunmadan koşabilsinler. Uzun süren her
//! fonksiyon bir [`ProgressSink`] alır; arayüz gerçeğini verir, testler sahtesini.

use crate::core::OpError;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Bundan fazlasına açıldığını iddia eden arşiv, kullanıcının diskini
/// doldurmasına izin verilmek yerine reddedilir. Gerçek bir save için fazlasıyla
/// cömert.
pub const MAX_EXTRACT_BYTES: u64 = 20 * 1024 * 1024 * 1024;

/// Varsayılan sıkıştırma seviyesi.
///
/// zlib'in varsayılanı 6 yerine 1: save içeriğine zaten sıkıştırılmış bölge
/// verisi hakim, seviye 6 pratikte aynı boyuttaki bir arşiv için ~%20 daha fazla
/// zaman harcıyor.
pub const DEFAULT_COMPRESSION_LEVEL: i64 = 1;

/// İlerleme bildirimi ve iptal kontrolü.
///
/// Python iki ayrı geri çağırma alıyordu (`progress` ve `is_cancelled`). Tek
/// çağrıda birleştirmek dosya başına iki yerine bir çağrı yapar ve daha önemlisi
/// "iptali kontrol etmeyi unutma" hatasını yapısal olarak imkânsız kılar: ilerleme
/// bildiren her yer aynı anda iptali de kontrol etmiş olur.
pub trait ProgressSink {
    /// İptal edilmişse [`OpError::Cancelled`] döner; çağıran `?` ile durur.
    fn tick(&self, done: u64, total: u64) -> Result<(), OpError>;
}

/// İlerlemeyi umursamayan çağıranlar için.
pub struct NoopSink;

impl ProgressSink for NoopSink {
    fn tick(&self, _done: u64, _total: u64) -> Result<(), OpError> {
        Ok(())
    }
}

/// Yedek ve dışa aktarmanın paylaştığı zaman damgası — ikisi birbirinden
/// ayrışamasın diye tek yerde.
///
/// Yerel saat kasıtlı: dosya adı kullanıcının saatini taşımalı. `chrono::Local`
/// tercih edildi çünkü `time` crate'inin `now_local()` fonksiyonu Unix'te çok
/// iş parçacıklı süreçlerde `IndeterminateOffset` ile başarısız oluyor — ve bu
/// uygulama işlemleri çalışan iş parçacıklarında koşturuyor.
pub fn timestamp_suffix() -> String {
    chrono::Local::now().format("%Y.%m.%d-%H.%M.%S").to_string()
}

/// Bir yolun dizin mi dosya mı olduğu. [`unique_path`] için gerekli.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathKind {
    Dir,
    File,
}

/// `path`'i, doluysa `path_2` / `path_3` ... biçiminde boş bir adı döndürür.
///
/// Zaman damgasının çözünürlüğü bir saniye, dolayısıyla aynı saniyede alınan iki
/// yedek aksi halde çakışırdı.
///
/// **Python'dan bilinçli sapma.** Orada `os.path.splitext` kullanılıyor ve o da
/// *son* noktadan bölüyor. Yedek klasörü adı `SaveA_backup_2026.08.16-14.30.00`
/// biçiminde olduğu için çakışmada `SaveA_backup_2026.08.16-14.30_2.00` gibi
/// bozuk bir ad üretiliyordu. Burada uzantı ayrıştırması yalnızca dosyalarda
/// yapılır; zip adlandırması Python ile birebir aynı kalır.
pub fn unique_path(path: impl Into<PathBuf>, kind: PathKind) -> PathBuf {
    let path = path.into();
    if !path.exists() {
        return path;
    }

    let parent = path.parent().map(Path::to_path_buf).unwrap_or_default();
    let name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();

    let (stem, extension) = match kind {
        PathKind::Dir => (name.as_str(), ""),
        // `rfind` yerine `i > 0` koşulu: ".bashrc" tümüyle isimdir, uzantı değil.
        // Python'un splitext'i de böyle davranır.
        PathKind::File => match name.rfind('.') {
            Some(index) if index > 0 => (&name[..index], &name[index..]),
            _ => (name.as_str(), ""),
        },
    };

    (2u64..)
        .map(|counter| parent.join(format!("{stem}_{counter}{extension}")))
        .find(|candidate| !candidate.exists())
        .expect("sayaç sınırsız")
}

/// Düşerken hedefi silen nöbetçi.
///
/// Python `try/except BaseException: rmtree(); raise` yazıyordu. RAII bundan
/// üstün: erken `?` dönüşünde de, panikte de çalışır ve çağrı yerinde unutulamaz.
struct CleanupGuard {
    path: PathBuf,
    kind: PathKind,
    armed: bool,
}

impl CleanupGuard {
    fn arm(path: &Path, kind: PathKind) -> Self {
        Self {
            path: path.to_path_buf(),
            kind,
            armed: true,
        }
    }

    fn disarm(mut self) {
        self.armed = false;
    }
}

impl Drop for CleanupGuard {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        // Temizlik sırasındaki hata yutulur: zaten bir başarısızlığı ele alıyoruz,
        // onun üstüne ikinci bir hata bindirmek kullanıcıya bir şey kazandırmaz.
        let _ = match self.kind {
            PathKind::Dir => fs::remove_dir_all(&self.path),
            PathKind::File => fs::remove_file(&self.path),
        };
    }
}

fn walk(root: &Path, want_dirs: bool) -> Result<Vec<PathBuf>, OpError> {
    let mut found = Vec::new();
    // `sort_by_file_name`: ilerleme bildirimi ve testler deterministik olsun.
    // Sembolik bağlar izlenmez (walkdir varsayılanı), Python'un `os.walk`'ı gibi.
    for entry in WalkDir::new(root).sort_by_file_name() {
        let entry = entry.map_err(|err| {
            let path = err.path().unwrap_or(root).to_path_buf();
            OpError::Io {
                path,
                source: err
                    .into_io_error()
                    .unwrap_or_else(|| io::Error::other("dizin ağacında döngü var")),
            }
        })?;
        let is_dir = entry.file_type().is_dir();
        if is_dir == want_dirs && entry.depth() > 0 {
            found.push(entry.into_path());
        }
    }
    Ok(found)
}

fn walk_files(root: &Path) -> Result<Vec<PathBuf>, OpError> {
    walk(root, false)
}

fn walk_dirs(root: &Path) -> Result<Vec<PathBuf>, OpError> {
    walk(root, true)
}

/// `shutil.copy2` paritesi için değişiklik zamanını taşır.
///
/// `fs::copy` izin bitlerini kopyalar ama mtime'ı kopyalamaz. Başarısızlık
/// **yedeği düşürmez**: içeriği doğru ama zaman damgası yanlış bir yedek, hiç
/// yedek olmamasından çok daha iyidir. Windows'ta salt-okunur bir kaynağın
/// kopyası da salt-okunur olur ve mtime yazımı reddedilir; o durumda bayrak
/// geçici olarak kaldırılıp geri konur.
fn copy_mtime(source: &Path, target: &Path) {
    let Ok(metadata) = fs::metadata(source) else {
        return;
    };
    let mtime = filetime::FileTime::from_last_modification_time(&metadata);

    if filetime::set_file_mtime(target, mtime).is_ok() {
        return;
    }
    if retry_without_readonly(target, mtime) {
        return;
    }

    log::warn!(
        "Değişiklik zamanı taşınamadı: {} -> {}",
        source.display(),
        target.display()
    );
}

/// Salt-okunur bayrağını geçici olarak kaldırıp mtime'ı yeniden dener.
///
/// Yalnızca Windows'ta derlenir. Unix'te `set_readonly(false)` dosyayı **herkese
/// yazılabilir** yapar (clippy `permissions_set_readonly_false` bunu haklı olarak
/// uyarıyor) ve zaten gereksizdir: orada mtime yazmak için dosyanın yazma izni
/// değil, sahipliği aranır.
#[cfg(windows)]
fn retry_without_readonly(target: &Path, mtime: filetime::FileTime) -> bool {
    let Ok(metadata) = fs::metadata(target) else {
        return false;
    };
    let mut permissions = metadata.permissions();
    if !permissions.readonly() {
        return false;
    }

    permissions.set_readonly(false);
    if fs::set_permissions(target, permissions).is_err() {
        return false;
    }

    let succeeded = filetime::set_file_mtime(target, mtime).is_ok();

    // Bayrağı geri koy: kopya kaynağıyla aynı izinlere sahip kalmalı.
    if let Ok(metadata) = fs::metadata(target) {
        let mut permissions = metadata.permissions();
        permissions.set_readonly(true);
        let _ = fs::set_permissions(target, permissions);
    }
    succeeded
}

#[cfg(not(windows))]
fn retry_without_readonly(_target: &Path, _mtime: filetime::FileTime) -> bool {
    false
}

/// Bir save dizinini kopyalar.
///
/// İptalde veya hatada yarım kalan hedef silinir. Geride bırakmak, save
/// klasörüne yarım kopyalanmış bir dizin koymak demektir ve uygulama (ile oyun)
/// onu gerçek bir save gibi listeler.
pub fn copy_save(
    source: &Path,
    destination: &Path,
    sink: &dyn ProgressSink,
) -> Result<(), OpError> {
    let files = walk_files(source)?;
    let total = files.len() as u64;

    let guard = CleanupGuard::arm(destination, PathKind::Dir);
    fs::create_dir_all(destination).map_err(OpError::io(destination))?;

    for (index, file) in files.iter().enumerate() {
        sink.tick(index as u64 + 1, total)?;

        let relative = file
            .strip_prefix(source)
            .expect("walkdir kökün altındaki yolları üretir");
        let target = destination.join(relative);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(OpError::io(parent))?;
        }
        fs::copy(file, &target).map_err(OpError::io(&target))?;
        copy_mtime(file, &target);
    }

    // Yalnızca dosyaları gezmek boş dizinleri düşürürdü.
    for directory in walk_dirs(source)? {
        let relative = directory
            .strip_prefix(source)
            .expect("walkdir kökün altındaki yolları üretir");
        let target = destination.join(relative);
        fs::create_dir_all(&target).map_err(OpError::io(&target))?;
    }

    guard.disarm();
    Ok(())
}

/// Bir save dizinini siler.
///
/// Bilerek iptal edilebilir değil: yarıda durmak kısmen silinmiş bir save
/// bırakır ve bu, işi bitirmekten daha kötüdür.
pub fn delete_save(source: &Path, sink: &dyn ProgressSink) -> Result<(), OpError> {
    fs::remove_dir_all(source).map_err(OpError::io(source))?;
    // İptal edilemeyen bir işlemde `tick`'in iptal sinyali anlamsız; yalnızca
    // ilerleme bildirmek için çağrılıyor.
    let _ = sink.tick(1, 1);
    Ok(())
}

/// Zip girdi adı: her platformda `/` ile ayrılmalı.
///
/// Windows'ta `Path` bileşenleri `\` ile birleşir ve öyle yazılan bir arşiv
/// Linux/macOS'ta tek bir uzun dosya adı gibi görünür. Python'un
/// `zipfile.write`'ı bu dönüşümü kendiliğinden yapıyordu.
fn zip_entry_name(relative: &Path) -> String {
    relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

/// Bir save dizinini zip'ler.
pub fn export_save(
    source: &Path,
    zip_path: &Path,
    compression_level: i64,
    sink: &dyn ProgressSink,
) -> Result<(), OpError> {
    let files = walk_files(source)?;
    let total = files.len() as u64;
    // Arşivin kökünde save adının kalması için bir üst dizinden göreceleştiriyoruz.
    let base = source.parent().unwrap_or(source);

    let guard = CleanupGuard::arm(zip_path, PathKind::File);
    let file = fs::File::create(zip_path).map_err(OpError::io(zip_path))?;
    let mut writer = zip::ZipWriter::new(io::BufWriter::new(file));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .compression_level(Some(compression_level));

    for (index, path) in files.iter().enumerate() {
        sink.tick(index as u64 + 1, total)?;

        let relative = path
            .strip_prefix(base)
            .expect("walkdir kökün altındaki yolları üretir");
        writer.start_file(zip_entry_name(relative), options)?;
        let mut input = fs::File::open(path).map_err(OpError::io(path))?;
        // Akışla kopyalanır: büyük bir bölge dosyası belleğe alınmaz.
        io::copy(&mut input, &mut writer).map_err(OpError::io(path))?;
    }

    writer.finish()?;
    guard.disarm();
    Ok(())
}

/// Arşivdeki üst düzey adlardan `target_dir` içinde zaten var olanlar.
///
/// **Her** üst düzey girdi kontrol edilir. Yalnızca ilkine bakmak (Python'un ilk
/// hali böyleydi), çok köklü bir arşivin ya da ilk girdisi serbest bir dosya olan
/// arşivin var olan save'lerin üzerine uyarısız yazmasına izin veriyordu.
pub fn archive_conflicts(zip_path: &Path, target_dir: &Path) -> Result<Vec<String>, OpError> {
    let file = fs::File::open(zip_path).map_err(OpError::io(zip_path))?;
    let archive = zip::ZipArchive::new(io::BufReader::new(file))?;

    let mut top_level = std::collections::BTreeSet::new();
    for name in archive.file_names() {
        if name.trim().is_empty() {
            continue;
        }
        if let Some(first) = name.replace('\\', "/").split('/').next()
            && !first.is_empty()
        {
            top_level.insert(first.to_string());
        }
    }

    Ok(top_level
        .into_iter()
        .filter(|name| target_dir.join(name).exists())
        .collect())
}

/// Arşivin açıldığında kaplayacağı toplam boyut.
pub fn archive_uncompressed_size(zip_path: &Path) -> Result<u64, OpError> {
    let file = fs::File::open(zip_path).map_err(OpError::io(zip_path))?;
    let mut archive = zip::ZipArchive::new(io::BufReader::new(file))?;
    let mut total = 0u64;
    for index in 0..archive.len() {
        total = total.saturating_add(archive.by_index(index)?.size());
    }
    Ok(total)
}

/// Yeni oluşturulan üst düzey girdileri, işlem yarıda kalırsa geri alan nöbetçi.
///
/// Bu **güvenli**, çünkü içe aktarma her zaman [`archive_conflicts`] kontrolünden
/// sonra başlar: hedefte çakışan bir üst düzey ad olmadığı garanti, dolayısıyla
/// sildiğimiz her şey bu işlemin kendi ürettiği şeydir.
struct RollbackGuard {
    target_dir: PathBuf,
    roots: std::collections::BTreeSet<String>,
    armed: bool,
}

impl RollbackGuard {
    fn new(target_dir: &Path) -> Self {
        Self {
            target_dir: target_dir.to_path_buf(),
            roots: std::collections::BTreeSet::new(),
            armed: true,
        }
    }

    fn record(&mut self, relative: &Path) {
        if let Some(first) = relative.components().next() {
            self.roots
                .insert(first.as_os_str().to_string_lossy().into_owned());
        }
    }

    fn disarm(mut self) {
        self.armed = false;
    }
}

impl Drop for RollbackGuard {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        for root in &self.roots {
            let path = self.target_dir.join(root);
            let _ = if path.is_dir() {
                fs::remove_dir_all(&path)
            } else {
                fs::remove_file(&path)
            };
        }
    }
}

/// Bir arşivi doğruladıktan sonra `target_dir` içine açar.
///
/// **Python'dan bilinçli sapma.** Orada CPython'un `ZipFile._extract_member`
/// fonksiyonunun sürücü harflerini ve `..` bileşenlerini sessizce kırpmasına
/// güveniliyordu; kaçmaya çalışan bir girdi sessizce hedefin içine düşüyordu.
/// `enclosed_name()` böyle bir girdide `None` döndürür ve biz arşivi
/// reddediyoruz: bozuk ya da kötü niyetli bir arşivin kullanıcıdan gizlenmesi
/// için bir neden yok.
pub fn import_save(
    zip_path: &Path,
    target_dir: &Path,
    max_bytes: u64,
    sink: &dyn ProgressSink,
) -> Result<(), OpError> {
    let file = fs::File::open(zip_path).map_err(OpError::io(zip_path))?;
    let mut archive = zip::ZipArchive::new(io::BufReader::new(file))?;

    if archive.is_empty() {
        return Err(OpError::EmptyArchive);
    }

    // Boyut önden toplanır: zip bombası tek bir bayt yazılmadan reddedilsin.
    let mut total_size = 0u64;
    for index in 0..archive.len() {
        total_size = total_size.saturating_add(archive.by_index(index)?.size());
    }
    if total_size > max_bytes {
        return Err(OpError::TooLarge {
            actual: total_size,
            limit: max_bytes,
        });
    }

    let total = archive.len() as u64;
    let mut guard = RollbackGuard::new(target_dir);

    for index in 0..archive.len() {
        sink.tick(index as u64 + 1, total)?;

        let mut entry = archive.by_index(index)?;
        let relative = entry
            .enclosed_name()
            .ok_or_else(|| OpError::UnsafePath(entry.name().to_string()))?;
        guard.record(&relative);

        let output = target_dir.join(&relative);
        if entry.is_dir() {
            fs::create_dir_all(&output).map_err(OpError::io(&output))?;
            continue;
        }
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent).map_err(OpError::io(parent))?;
        }
        let mut target = fs::File::create(&output).map_err(OpError::io(&output))?;
        io::copy(&mut entry, &mut target).map_err(OpError::io(&output))?;
    }

    guard.disarm();
    Ok(())
}
