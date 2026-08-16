# 7 Days To Backup (Rust)

[7DaysToBackup](https://github.com/umityatarkalkmaz/7DaysToBackup) aracının Rust ile
yeniden yazımı. 7 Days To Die save dosyalarını yedekleme, dışa aktarma ve silme
işlemlerini tek bir masaüstü arayüzünden yapmayı hedefler.

Python sürümü PySide6 kullanıyor; bu sürüm [egui/eframe](https://github.com/emilk/egui)
ile yazılıyor. Böylece çıktı, sistem üzerinde GTK/Qt/webview bağımlılığı olmayan
tek bir çalıştırılabilir dosya oluyor.

> **Durum:** Erken aşama. Şu an yalnızca pencere iskeleti var; yedekleme mantığı
> henüz taşınmadı.

## Gereksinimler

- Rust stable (1.97 veya üstü)
- Linux'ta derlemek için: `libxkbcommon`, `xkeyboard-config` ve bir GL/Vulkan sürücüsü

## Çalıştırma

```bash
cargo run
```

Release derlemesi:

```bash
cargo build --release
```

Üretilen dosya `target/release/seven-days-to-backup` (Windows'ta `.exe`) yolunda.

## Geliştirme

```bash
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test
```

## Lisans

MIT. Ayrıntı için [LICENSE](LICENSE).
