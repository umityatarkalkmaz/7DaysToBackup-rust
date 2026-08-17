# 7 Days To Backup (Rust)

7 Days to Die save dosyalarını yedekleme, dışa aktarma, içe aktarma ve silme
işlemlerini tek bir masaüstü arayüzünden yapar.

Rust ve [egui/eframe](https://github.com/emilk/egui) ile yazıldı. Çıktı, hedef
makinede hiçbir çalışma zamanı gerektirmeyen tek bir çalıştırılabilir dosya.

## Özellikler

- Map ve save listesi
- Tarih damgalı yedekleme (`SaveA_backup_2026.08.16-14.30.00`)
- Onay isteyen silme
- Yedek geçmişi ve geri yükleme
- Çoklu save seçimi (Ctrl ile ekle, Shift ile aralık)
- Otomatik yedekleme (zamanlayıcı ve eski yedekleri budama)
- Zip'e dışa aktarma (masaüstüne)
- Zip'ten içe aktarma, çakışma ve boyut denetimiyle
- İlerleme göstergesi ve iptal
- Koyu tema, Türkçe/İngilizce
- Özel save klasörü ayarı
- Ayarlanabilir arayüz ölçeği (yüksek çözünürlüklü ekranlar için)

## Kurulum

[Yayımlananlar](https://github.com/umityatarkalkmaz/7DaysToBackup-rust/releases)
bölümünden platformunuza uygun dosyayı indirin:

| Platform | Dosya |
|---|---|
| Windows | `7DaysToBackup-rust-windows.zip` |
| Linux | `7DaysToBackup-rust-linux.tar.gz` |
| macOS | `7DaysToBackup-rust-macos.zip` |

**Windows:** zip'e çift tıklayıp içindeki `7DaysToBackup.exe`'yi çıkarın.

**macOS:** zip'i açın, çıkan `7DaysToBackup.app`'i Uygulamalar klasörüne sürükleyin.
İlk açışta **sağ tık → Aç** deyin (uygulama imzasız olduğu için çift tıklama
reddedilir). Sürüm **yalnızca Apple Silicon (arm64)** içindir.

**Linux:** arşivi açın, sonra çalıştırın:

```bash
tar -xzf 7DaysToBackup-rust-linux.tar.gz
./7DaysToBackup
```

> Linux ve macOS ikilileri neden arşiv içinde: GitHub yayın varlıkları düz
> dosyadır ve çalıştırma iznini taşımaz. Doğrudan indirilen uzantısız bir dosyayı
> macOS metin belgesi sanıp TextEdit'te açıyordu. `tar` izni kaydettiği için
> arşivden çıkan dosya doğrudan çalışır.

> **macOS'ta "geliştirici doğrulanamadı" uyarısı:** uygulama Apple tarafından
> noterlenmiş değil (bu ücretli bir geliştirici hesabı gerektiriyor). Sağ tık →
> **Aç** yeterli; inatçı bir uyarıda `xattr -cr 7DaysToBackup.app` çalıştırın.

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

## Arayüz küçük görünüyorsa

egui, masaüstünün yazı tipi boyutunu okumaz; ölçeği yalnızca pencere
yöneticisinin bildirdiği katsayıdan alır. Masaüstü ölçeklemesi %100 olan 1440p ve
üstü ekranlarda arayüz bu yüzden küçük kalabilir.

Ayarlar penceresindeki **Arayüz ölçeği** ile büyütebilirsiniz; değer
`config.json` içinde saklanır. `Ctrl` ve `+`/`-` de çalışır ama kalıcı değildir.
Ayar boşken uygulama monitör boyutuna bakıp kendisi bir tahmin yapar.

Ayarlar ve günlükler:

| | Windows | macOS | Linux |
|---|---|---|---|
| Ayarlar | `%APPDATA%\7DaysToBackup-rust` | `~/Library/Application Support/7DaysToBackup-rust` | `~/.config/7DaysToBackup-rust` |
| Günlük | `%LOCALAPPDATA%\7DaysToBackup-rust` | `~/Library/Logs/7DaysToBackup-rust` | `~/.local/state/7DaysToBackup-rust` |

Ayrıntılı günlük için `SEVENDAYS_DEBUG=1` ile çalıştırın.

## Davranış notları

Şaşırtabilecek ya da bilinçli olarak seçilmiş kararlar:

- **Yedekler save'lerin yanında durur, ama listede karışmaz.** Yedek klasörü
  save'lerle aynı yere yazılır ki oyun onu ayrı bir save olarak açabilsin;
  uygulamada kendi geçmiş sütununda, tarihiyle listelenir.
- **Geri yükleme yıkıcı değildir.** Save'in o anki hâli silinmez, yeni bir yedeğe
  dönüştürülür. Yanlış yedeği seçmek veri kaybettirmez.
- **Otomatik yedekleme araya girmez.** Elle başlattığınız bir işlem sürerken tur
  atlanır ve sonuç pencere açmadan durum satırına yazılır — oyun oynarken
  önünüze pencere çıkmaz. Geçmiş, belirlediğiniz sayıya budanır.
- **Seçim işlem sonrası korunur.** Yedekten sonra aynı save'i yeniden seçmeniz
  gerekmez; silinen bir save seçimden kendiliğinden düşer.
- **Save listesi yalnızca dizinleri gösterir.** Map klasörüne düşmüş başıboş bir
  dosya save gibi görünmez.
- **Hedefin dışına yazmaya çalışan zip reddedilir.** Yol içinde `..` taşıyan bir
  arşiv sessizce kırpılıp içeri alınmaz; reddedilir ve nedeni söylenir.
- **Arşiv, bildirdiği boyuttan fazlasını açamaz.** Arşivin kendi beyan ettiği
  boyut yalan olabilir, o yüzden sınır gerçekten diske yazılan bayt üzerinden de
  uygulanır.
- **Yarıda kalan içe aktarma geri alınır.** Yarım açılmış dosyalar geride kalmaz.
- **Yalnızca deflate ile sıkıştırılmış zip'ler açılabilir.** 7-Zip gibi bir araçla
  LZMA/zstd seçilerek üretilmiş bir arşiv reddedilir. Karşılığında ikili ~1,2 MB
  küçüldü ve arşiv ayrıştıran tek C kütüphanesi (`zstd-sys`) bağımlılıklardan çıktı.
- **Emoji içeren save adları boş kutu görünür.** İkili boyutunu düşürmek için
  yalnızca tek bir yazı tipi gömülüyor. Türkçe karakterler eksiksiz.

## Geliştirme

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test
```

Yapı:

```
src/
├── core/        GUI'siz çekirdek — paths, platform, config, log, ops, error
├── task.rs      arka plan işlemleri (mpsc + AtomicBool)
├── i18n.rs      metinler
└── ui/          egui katmanı — app, settings, theme
assets/
├── icon.svg     ikonun kaynağı — değiştirilecek yer burası
└── icon-*.png   ondan üretilmiş boyutlar (pencere ikonu ve macOS .icns)
```

İkon `assets/icon.svg`'den `resvg` ile rasterleştirilip depoya konur; üretim tek
seferliktir, rasterleştirici depoda durmaz. Yeniden üretmek için SVG'yi herhangi
bir SVG aracıyla 16/32/64/128/256/512/1024 boyutlarına verin.

> Wayland'de pencere ikonu görünmez — protokol istemcinin ikon atamasına izin
> vermiyor, orada bir `.desktop` dosyası gerekir. X11, Windows ve macOS'ta görünür.

`core/` ve `task.rs` içinde `egui`/`eframe` importu yoktur; bu sayede iş mantığı
ekransız test edilebilir.

## Lisans

MIT. Ayrıntı için [LICENSE](LICENSE).
