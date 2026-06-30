use std::borrow::Cow;

use anyhow::Result;
use gpui::App;

pub const JETBRAINS_MONO: &str = "JetBrains Mono";

pub fn register(cx: &mut App) -> Result<()> {
    cx.text_system().add_fonts(vec![
        font_bytes(include_bytes!(
            "../assets/fonts/jetbrains-mono/JetBrainsMono-Thin.ttf"
        )),
        font_bytes(include_bytes!(
            "../assets/fonts/jetbrains-mono/JetBrainsMono-ThinItalic.ttf"
        )),
        font_bytes(include_bytes!(
            "../assets/fonts/jetbrains-mono/JetBrainsMono-ExtraLight.ttf"
        )),
        font_bytes(include_bytes!(
            "../assets/fonts/jetbrains-mono/JetBrainsMono-ExtraLightItalic.ttf"
        )),
        font_bytes(include_bytes!(
            "../assets/fonts/jetbrains-mono/JetBrainsMono-Light.ttf"
        )),
        font_bytes(include_bytes!(
            "../assets/fonts/jetbrains-mono/JetBrainsMono-LightItalic.ttf"
        )),
        font_bytes(include_bytes!(
            "../assets/fonts/jetbrains-mono/JetBrainsMono-Regular.ttf"
        )),
        font_bytes(include_bytes!(
            "../assets/fonts/jetbrains-mono/JetBrainsMono-Italic.ttf"
        )),
        font_bytes(include_bytes!(
            "../assets/fonts/jetbrains-mono/JetBrainsMono-Medium.ttf"
        )),
        font_bytes(include_bytes!(
            "../assets/fonts/jetbrains-mono/JetBrainsMono-MediumItalic.ttf"
        )),
        font_bytes(include_bytes!(
            "../assets/fonts/jetbrains-mono/JetBrainsMono-SemiBold.ttf"
        )),
        font_bytes(include_bytes!(
            "../assets/fonts/jetbrains-mono/JetBrainsMono-SemiBoldItalic.ttf"
        )),
        font_bytes(include_bytes!(
            "../assets/fonts/jetbrains-mono/JetBrainsMono-Bold.ttf"
        )),
        font_bytes(include_bytes!(
            "../assets/fonts/jetbrains-mono/JetBrainsMono-BoldItalic.ttf"
        )),
        font_bytes(include_bytes!(
            "../assets/fonts/jetbrains-mono/JetBrainsMono-ExtraBold.ttf"
        )),
        font_bytes(include_bytes!(
            "../assets/fonts/jetbrains-mono/JetBrainsMono-ExtraBoldItalic.ttf"
        )),
    ])
}

fn font_bytes(bytes: &'static [u8]) -> Cow<'static, [u8]> {
    Cow::Borrowed(bytes)
}
