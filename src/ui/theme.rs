//! Koyu tema.
//!
//! Renkler Python sürümündeki `create_dark_palette()` fonksiyonundan birebir
//! alındı; iki uygulama yan yana açıldığında aynı görünsünler.

use egui::{Color32, FontData, FontDefinitions, FontFamily, Visuals};
use std::sync::Arc;

/// Gömülü fontun `FontDefinitions` içindeki adı.
const FONT: &str = "Ubuntu-Light";

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

/// Yalnızca kullanılan fontu yükler.
///
/// egui'nin varsayılanı dört fontu birden gömüyor: Ubuntu-Light, Hack
/// (monospace), NotoEmoji ve emoji-icon-font — toplamı ~1,4 MB ve hepsi
/// `include_bytes!` ile ikilinin içinde. Bu uygulama monospace metin de emoji de
/// göstermiyor.
///
/// **Bedeli:** save adında emoji varsa boş kutu ("tofu") görünür. Türkçe
/// karakterler Ubuntu-Light'ta tam olarak var.
pub fn fonts() -> FontDefinitions {
    let mut fonts = FontDefinitions::empty();
    fonts.font_data.insert(
        FONT.to_owned(),
        Arc::new(FontData::from_static(epaint_default_fonts::UBUNTU_LIGHT)),
    );

    // İki aile de aynı fonta bağlanıyor. `TextStyle::Monospace` varsayılan stil
    // haritasında duruyor ve boş bir aile istendiğinde egui hiçbir şey çizemez —
    // uygulama o stili bugün kullanmasa da bağlamak bedava.
    for family in [FontFamily::Proportional, FontFamily::Monospace] {
        fonts
            .families
            .entry(family)
            .or_default()
            .push(FONT.to_owned());
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
