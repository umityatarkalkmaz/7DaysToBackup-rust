# 7 Days To Backup (Rust)

[7DaysToBackup](https://github.com/umityatarkalkmaz/7DaysToBackup) aracının Rust ile
yeniden yazımı. 7 Days to Die save dosyalarını yedekleme, dışa aktarma, içe aktarma
ve silme işlemlerini tek bir masaüstü arayüzünden yapar.

Python sürümü PySide6 kullanıyor; bu sürüm [egui/eframe](https://github.com/emilk/egui)
ile yazıldı. Çıktı, hedef makinede Python, Qt veya webview gerektirmeyen tek bir
çalıştırılabilir dosya.

## Özellikler

- Map ve save listesi
- Tarih damgalı yedekleme (`SaveA_backup_2026.08.16-14.30.00`)
- Onay isteyen silme
- Zip'e dışa aktarma (masaüstüne)
- Zip'ten içe aktarma, çakışma ve boyut denetimiyle
- İlerleme göstergesi ve iptal
- Koyu tema, Türkçe/İngilizce
- Özel save klasörü ayarı

## Kurulum

[Yayımlananlar](https://github.com/umityatarkalkmaz/7DaysToBackup-rust/releases)
bölümünden platformunuza uygun dosyayı indirin:

| Platform | Dosya |
|---|---|
| Windows | `7DaysToBackup-rust-windows.exe` |
| Linux | `7DaysToBackup-rust-linux` |
| macOS | `7DaysToBackup-rust-macos` |

> EXE imzasız dağıtıldığı için bazı antivirüs programları uyarı verebilir. Bu,
> uygulamanın güvensiz olduğu anlamına gelmez; endişeniz varsa aşağıdaki adımlarla
> kendiniz derleyebilirsiniz.

## Kaynaktan derleme

Rust stable 1.97 veya üstü gerekir (`rust-toolchain.toml` sürümü sabitler).

```bash
git clone https://github.com/umityatarkalkmaz/7DaysToBackup-rust
cd 7DaysToBackup-rust
cargo run --release
```

Linux'ta derlemek için ek sistem paketleri gerekir:

```bash
# Arch
sudo pacman -S libxkbcommon libxkbcommon-x11 xkeyboard-config
# Debian/Ubuntu
sudo apt install libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev libgl1-mesa-dev
```

## Save klasörü nerede aranır

| Platform | Konum |
|---|---|
| Windows | `%APPDATA%\7DaysToDie\Saves` |
| macOS | `~/Library/Application Support/7DaysToDie/Saves` |
| Linux | `~/.local/share/7DaysToDie/Saves` |

Bulunamazsa Ayarlar penceresinden özel bir konum tanımlayabilirsiniz.

Ayarlar ve günlükler:

| | Windows | macOS | Linux |
|---|---|---|---|
| Ayarlar | `%APPDATA%\7DaysToBackup-rust` | `~/Library/Application Support/7DaysToBackup-rust` | `~/.config/7DaysToBackup-rust` |
| Günlük | `%LOCALAPPDATA%\7DaysToBackup-rust` | `~/Library/Logs/7DaysToBackup-rust` | `~/.local/state/7DaysToBackup-rust` |

Ayrıntılı günlük için `SEVENDAYS_DEBUG=1` ile çalıştırın.

## Python sürümüyle farklar

Davranışın büyük kısmı birebir aynı. Bilinçli olarak ayrılan yerler:

- **Ayarlar ayrı dizinde.** İki sürüm yan yana kurulabilsin diye Rust sürümü
  `7DaysToBackup-rust` klasörünü kullanır. Özel save yolunu bir kez daha
  girmeniz gerekir.
- **Hedefin dışına yazmaya çalışan zip reddedilir.** Python'un kullandığı
  `zipfile`, yol içindeki `..` bileşenlerini sessizce kırpıp dosyayı hedefin
  içine düşürüyordu. Burada arşiv reddedilir ve neden söylenir.
- **Yarıda kalan içe aktarma geri alınır.** Python yarım açılmış dosyaları
  bırakıyordu.
- **Çakışan yedek adı bozulmaz.** `os.path.splitext` son noktadan böldüğü için
  Python `SaveA_backup_2026.08.16-14.30_2.00` gibi bir ad üretebiliyordu.
- **Save listesi dizinlere süzülür.** Python'da map listesi süzülüyor ama save
  listesi süzülmüyordu; map klasörüne düşmüş başıboş bir dosya save gibi
  görünüyordu.
- **Seçim işlem sonrası korunur.** Python her işlemden sonra listeyi yenileyip
  seçimi düşürüyordu.

## Geliştirme

```bash
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Yapı:

```
src/
├── core/        GUI'siz çekirdek — paths, platform, config, log, ops, error
├── task.rs      arka plan işlemleri (mpsc + AtomicBool)
├── i18n.rs      metinler
└── ui/          egui katmanı — app, settings, theme
```

`core/` ve `task.rs` içinde `egui`/`eframe` importu yoktur; bu sayede iş mantığı
ekransız test edilebilir. Python tarafında `core/` paketinin Qt'siz tutulmasıyla
aynı kural.

## Lisans

MIT. Ayrıntı için [LICENSE](LICENSE).
