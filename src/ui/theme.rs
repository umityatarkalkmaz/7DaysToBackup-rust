//! Koyu tema.
//!
//! Renkler Python sürümündeki `create_dark_palette()` fonksiyonundan birebir
//! alındı; iki uygulama yan yana açıldığında aynı görünsünler.

use egui::{Color32, FontData, FontDefinitions, FontFamily, Visuals};
use std::sync::Arc;

/// Metin fontu.
const TEXT_FONT: &str = "Ubuntu-Light";

/// Simge fontu — ayarlar düğmesindeki "⚙" (U+2699) için.
///
/// Ubuntu-Light'ta o glif yok ve gömülü dört fonttan yalnızca bunda var. Yedek
/// zincirinde metin fontundan **sonra** geliyor: ortak karakterlerde metin fontu
/// kazanmalı.
const ICON_FONT: &str = "emoji-icon-font";

/// `QPalette::Window` — panel zemini.
const WINDOW: Color32 = Color32::from_rgb(35, 39, 46);
/// `QPalette::Base` — liste ve metin kutusu zemini.
const BASE: Color32 = Color32::from_rgb(42, 46, 56);
/// `QPalette::Button` — düğme yüzeyi.
const BUTTON: Color32 = Color32::from_rgb(58, 63, 75);
/// `QPalette::Highlight` — seçili öğe.
const HIGHLIGHT: Color32 = Color32::from_rgb(68, 74, 86);
/// `QPalette::WindowText` / `Text` / `ButtonText` — hepsi aynı renk.
const TEXT: Color32 = Color32::from_rgb(245, 246, 250);

/// Durum etiketi için kırmızımsı ton (`window.py` içindeki `color: #e08f8f`).
pub const STATUS_TEXT: Color32 = Color32::from_rgb(224, 143, 143);

/// Yalnızca gereken fontları yükler.
///
/// egui'nin varsayılanı dört fontu birden gömüyor: Ubuntu-Light (362 KB), Hack
/// (309 KB, monospace), NotoEmoji (419 KB) ve emoji-icon-font (324 KB) — toplamı
/// ~1,4 MB ve hepsi `include_bytes!` ile ikilinin içinde.
///
/// Burada ikisi yükleniyor: metin fontu ve simge fontu. Düşen ~728 KB, Hack ile
/// NotoEmoji; uygulama ne monospace metin ne de emoji gösteriyor.
///
/// **Simge fontu bir kez atılmıştı ve ayarlar düğmesi sessizce boşaldı.** Eksik
/// bir glif hata vermez, sadece çizilmez; `every_glyph_the_interface_uses_is_covered`
/// testi o yüzden var.
///
/// **Bedeli:** save adında emoji varsa boş kutu ("tofu") görünebilir. Türkçe
/// karakterler Ubuntu-Light'ta tam olarak var.
pub fn fonts() -> FontDefinitions {
    let mut fonts = FontDefinitions::empty();
    for (name, bytes) in [
        (TEXT_FONT, epaint_default_fonts::UBUNTU_LIGHT),
        (ICON_FONT, epaint_default_fonts::EMOJI_ICON),
    ] {
        fonts
            .font_data
            .insert(name.to_owned(), Arc::new(FontData::from_static(bytes)));
    }

    // İki aile de aynı zincire bağlanıyor. `TextStyle::Monospace` varsayılan stil
    // haritasında duruyor ve boş bir aile istendiğinde egui hiçbir şey çizemez —
    // uygulama o stili bugün kullanmasa da bağlamak bedava.
    for family in [FontFamily::Proportional, FontFamily::Monospace] {
        let chain = fonts.families.entry(family).or_default();
        chain.push(TEXT_FONT.to_owned());
        chain.push(ICON_FONT.to_owned());
    }

    fonts
}

pub fn dark_visuals() -> Visuals {
    let mut visuals = Visuals::dark();

    visuals.panel_fill = WINDOW;
    visuals.window_fill = WINDOW;
    visuals.extreme_bg_color = BASE;
    visuals.faint_bg_color = BASE;
    visuals.override_text_color = Some(TEXT);

    visuals.selection.bg_fill = HIGHLIGHT;
    visuals.selection.stroke.color = TEXT;

    // Qt'de düğme, üzerine gelince ve basılınca aynı fırçayı kullanır; egui
    // durumları ayrı tuttuğu için hepsini elle eşliyoruz.
    for widget in [
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        widget.bg_fill = BUTTON;
        widget.weak_bg_fill = BUTTON;
    }
    visuals.widgets.noninteractive.bg_fill = WINDOW;
    visuals.widgets.noninteractive.weak_bg_fill = WINDOW;

    visuals
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_glyph_the_interface_uses_is_covered() {
        // **Regresyon.** Varsayılan font kümesi kapatılınca ayarlar düğmesindeki
        // "⚙" (U+2699) sessizce boşaldı: o glif yalnızca emoji-icon-font'ta var
        // ve dört fontun hepsi birden atılmıştı. Hiçbir test görmedi, çünkü
        // eksik glif bir hata değil, sadece çizilmeyen bir şey.
        //
        // Bu test glife değil kurala bakıyor: arayüzün gösterdiği her metin,
        // yüklü fontlarca kapsanmak zorunda. Yeni bir dizge özel bir karakter
        // getirirse burada patlar.
        use skrifa::MetadataProvider as _;

        let definitions = fonts();
        let faces: Vec<skrifa::FontRef<'_>> = definitions
            .font_data
            .values()
            .map(|data| skrifa::FontRef::new(&data.font).expect("gömülü font ayrıştırılamalı"))
            .collect();
        assert!(!faces.is_empty());

        let covered = |glyph: char| faces.iter().any(|face| face.charmap().map(glyph).is_some());

        for lang in crate::i18n::Lang::ALL {
            for (name, text) in lang.strings().fields() {
                // Kontrol karakterleri çizilmez, fontta aranmaz.
                for glyph in text.chars().filter(|glyph| !glyph.is_control()) {
                    assert!(
                        covered(glyph),
                        "{lang:?}/{name}: {glyph:?} (U+{:04X}) yüklü fontların hiçbirinde yok -> {text:?}",
                        glyph as u32
                    );
                }
            }
        }
    }

    #[test]
    fn every_font_family_resolves_to_a_loaded_font() {
        // Varsayılan font kümesi kapatıldı; boş kalan bir aile istendiğinde egui
        // hiçbir şey çizemez ve arayüz sessizce boşalır.
        let fonts = fonts();
        for family in [FontFamily::Proportional, FontFamily::Monospace] {
            let names = &fonts.families[&family];
            assert!(!names.is_empty(), "{family:?} ailesi boş");
            for name in names {
                assert!(fonts.font_data.contains_key(name), "{name} yüklenmemiş");
            }
        }
    }

    #[test]
    fn matches_the_python_palette() {
        let visuals = dark_visuals();
        assert_eq!(visuals.panel_fill, Color32::from_rgb(35, 39, 46));
        assert_eq!(visuals.extreme_bg_color, Color32::from_rgb(42, 46, 56));
        assert_eq!(visuals.selection.bg_fill, Color32::from_rgb(68, 74, 86));
        assert_eq!(
            visuals.override_text_color,
            Some(Color32::from_rgb(245, 246, 250))
        );
    }

    #[test]
    fn every_interactive_state_uses_the_button_colour() {
        let visuals = dark_visuals();
        let button = Color32::from_rgb(58, 63, 75);
        assert_eq!(visuals.widgets.inactive.bg_fill, button);
        assert_eq!(visuals.widgets.hovered.bg_fill, button);
        assert_eq!(visuals.widgets.active.bg_fill, button);
    }
}
