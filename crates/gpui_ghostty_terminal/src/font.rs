pub fn default_terminal_font() -> gpui::Font {
    let family = if cfg!(target_os = "macos") {
        "Menlo"
    } else if cfg!(target_os = "windows") {
        "Consolas"
    } else {
        "DejaVu Sans Mono"
    };

    let fallbacks = gpui::FontFallbacks::from_fonts(vec![
        "SF Mono".to_string(),
        "Menlo".to_string(),
        "Monaco".to_string(),
        "Consolas".to_string(),
        "Cascadia Mono".to_string(),
        "DejaVu Sans Mono".to_string(),
        "Noto Sans Mono".to_string(),
        "JetBrains Mono".to_string(),
        "Fira Mono".to_string(),
        "Sarasa Mono SC".to_string(),
        "Sarasa Term SC".to_string(),
        "Sarasa Mono J".to_string(),
        "Noto Sans Mono CJK SC".to_string(),
        "Noto Sans Mono CJK JP".to_string(),
        "Source Han Mono SC".to_string(),
        "WenQuanYi Zen Hei Mono".to_string(),
        "Apple Color Emoji".to_string(),
        "Noto Color Emoji".to_string(),
        "Segoe UI Emoji".to_string(),
    ]);

    let mut font = gpui::font(family);
    font.fallbacks = Some(fallbacks);
    font
}

pub fn default_terminal_font_features() -> gpui::FontFeatures {
    use std::sync::Arc;
    gpui::FontFeatures(Arc::new(vec![
        ("calt".to_string(), 0),
        ("liga".to_string(), 0),
        ("kern".to_string(), 0),
    ]))
}
