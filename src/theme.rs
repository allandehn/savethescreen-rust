//! The open Omarchy theme, read while the app is running.
//!
//! Colors come from `~/.local/state/omarchy/current/theme/colors.toml`.
//! Text size comes from `[font] base-size` in the theme `shell.toml`, with
//! `~/.config/omarchy/shell.toml` on top, the same way the shell does.
//! The face is whatever `monospace` resolves to, which `omarchy font set` writes.
//! A missing file keeps the built-in shelf colors.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::UNIX_EPOCH;

#[derive(Clone)]
struct Theme {
    bg: [f32; 4],
    raise: [f32; 4],
    ink: [u8; 4],
    muted: [u8; 4],
    ink_soft: [u8; 4],
    brand: [u8; 4],
    accent: [u8; 4],
    line: [f32; 4],
    hover: [f32; 4],
    scrim: [f32; 4],
    mark: [f32; 4],
    code_ink: [u8; 3],
    code_wash: [f32; 4],
    code_well: [f32; 4],
    ui_font: String,
    mono_font: String,
    /// 1.0 when the shell text size is 12. Larger base sizes grow the type.
    scale: f32,
    /// Shell `font.body-small`, in px. 11 when the base size is 12.
    body_small: f32,
    /// Shell bar fill. Falls back to the card color.
    bar_bg: [f32; 4],
    /// Shell bar label. Falls back to the soft foreground.
    bar_ink: [u8; 4],
    /// Card-pill fill, when the palette names a highlight background and foreground.
    highlight_bg: Option<[f32; 4]>,
    /// Card-pill letters, paired with `highlight_bg`.
    highlight_ink: Option<[u8; 3]>,
    stamp: u32,
}

struct State {
    theme: Theme,
    fingerprint: u128,
    loaded: bool,
}

fn state() -> &'static Mutex<State> {
    static STATE: OnceLock<Mutex<State>> = OnceLock::new();
    STATE.get_or_init(|| {
        Mutex::new(State {
            theme: fallback("Adwaita Sans", "Adwaita Mono"),
            fingerprint: 0,
            loaded: false,
        })
    })
}

fn lock() -> std::sync::MutexGuard<'static, State> {
    state().lock().unwrap_or_else(|err| err.into_inner())
}

fn ensure(state: &mut State) {
    if state.loaded {
        return;
    }
    state.fingerprint = fingerprint();
    state.theme = load_from_disk();
    state.loaded = true;
}

/// True when the theme files changed and the shelf should draw again.
pub fn reload_if_changed() -> bool {
    let mut state = lock();
    ensure(&mut state);
    let next = fingerprint();
    if next == state.fingerprint {
        return false;
    }
    state.fingerprint = next;
    state.theme = load_from_disk();
    true
}

fn with<T>(f: impl FnOnce(&Theme) -> T) -> T {
    let mut state = lock();
    ensure(&mut state);
    f(&state.theme)
}

pub fn bg() -> [f32; 4] {
    with(|theme| theme.bg)
}
pub fn raise() -> [f32; 4] {
    with(|theme| theme.raise)
}
pub fn ink() -> [u8; 4] {
    with(|theme| theme.ink)
}
pub fn muted() -> [u8; 4] {
    with(|theme| theme.muted)
}
pub fn ink_soft() -> [u8; 4] {
    with(|theme| theme.ink_soft)
}
pub fn brand() -> [u8; 4] {
    with(|theme| theme.brand)
}
pub fn accent() -> [u8; 4] {
    with(|theme| theme.accent)
}
pub fn line() -> [f32; 4] {
    with(|theme| theme.line)
}
pub fn hover() -> [f32; 4] {
    with(|theme| theme.hover)
}
pub fn scrim() -> [f32; 4] {
    with(|theme| theme.scrim)
}
pub fn mark() -> [f32; 4] {
    with(|theme| theme.mark)
}
pub fn code_ink() -> [u8; 3] {
    with(|theme| theme.code_ink)
}
pub fn code_wash() -> [f32; 4] {
    with(|theme| theme.code_wash)
}
pub fn code_well() -> [f32; 4] {
    with(|theme| theme.code_well)
}
pub fn ui_font() -> String {
    with(|theme| theme.ui_font.clone())
}
pub fn mono_font() -> String {
    with(|theme| theme.mono_font.clone())
}
pub fn stamp() -> u32 {
    with(|theme| theme.stamp)
}

/// The shell's smaller body size, in px. An override in `shell.toml` wins.
pub fn body_small() -> f32 {
    with(|theme| theme.body_small)
}

/// Fill for the shelf bar. `[bar] background` in the shell theme, or the card color.
pub fn bar_bg() -> [f32; 4] {
    with(|theme| theme.bar_bg)
}

/// Label color for the shelf bar. `[bar] text` in the shell theme, or the soft foreground.
pub fn bar_ink() -> [u8; 4] {
    with(|theme| theme.bar_ink)
}

/// Highlight fill and letters for the card pill. `None` until the palette names both.
pub fn highlight_pair() -> Option<([f32; 4], [u8; 3])> {
    with(|theme| theme.highlight_bg.zip(theme.highlight_ink))
}

/// A size written for a shell text size of 12, scaled to the current one.
pub fn px(size: f32) -> f32 {
    with(|theme| size * theme.scale)
}

fn home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn fingerprint() -> u128 {
    let theme = home().join(".local/state/omarchy/current/theme");
    file_stamp(&theme.join("colors.toml"))
        ^ file_stamp(&theme.join("shell.toml")).rotate_left(11)
        ^ file_stamp(&home().join(".config/omarchy/shell.toml")).rotate_left(23)
        ^ file_stamp(&home().join(".config/fontconfig/fonts.conf")).rotate_left(37)
}

fn file_stamp(path: &Path) -> u128 {
    let Ok(meta) = fs::metadata(path) else {
        return 0;
    };
    let modified = meta
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|time| time.as_nanos())
        .unwrap_or(0);
    modified ^ (u128::from(meta.len()) << 64)
}

fn load_from_disk() -> Theme {
    let theme_dir = home().join(".local/state/omarchy/current/theme");
    let colors = fs::read_to_string(theme_dir.join("colors.toml")).unwrap_or_default();
    let theme_shell = fs::read_to_string(theme_dir.join("shell.toml")).unwrap_or_default();
    let user_shell =
        fs::read_to_string(home().join(".config/omarchy/shell.toml")).unwrap_or_default();
    let font = mono_family();
    theme_from(&colors, &theme_shell, &user_shell, &font)
}

fn mono_family() -> String {
    let output = std::process::Command::new("fc-match")
        .args(["monospace", "-f", "%{family}"])
        .output();
    let Ok(output) = output else {
        return String::new();
    };
    if !output.status.success() {
        return String::new();
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .unwrap_or("")
        .split(',')
        .next()
        .unwrap_or("")
        .trim()
        .to_string()
}

fn theme_from(colors: &str, theme_shell: &str, user_shell: &str, font: &str) -> Theme {
    let palette = assigns(colors);
    let mut shell = assigns(theme_shell);
    for (key, value) in assigns(user_shell) {
        shell.insert(key, value);
    }
    let font = font.trim();
    let (ui, mono) = if font.is_empty() {
        ("Adwaita Sans".to_string(), "Adwaita Mono".to_string())
    } else {
        (font.to_string(), font.to_string())
    };
    let mut theme = fallback(&ui, &mono);
    if !palette.is_empty() {
        let background = rgb(&palette, "background", [0x12, 0x11, 0x0e]);
        let darker = rgb(&palette, "darker_background", background);
        let foreground = rgb(&palette, "foreground", [0xec, 0xe7, 0xdc]);
        let bright = rgb(&palette, "bright_foreground", foreground);
        let light = rgb(&palette, "light_foreground", foreground);
        let accent = rgb(&palette, "accent", [0x7d, 0xae, 0xa3]);
        let yellow = rgb(&palette, "yellow", accent);

        theme.bg = rgb_f(darker, 1.0);
        theme.raise = rgb_f(background, 1.0);
        theme.ink = rgba(bright, 255);
        theme.muted = rgba(light, 255);
        theme.ink_soft = rgba(foreground, 220);
        theme.brand = rgba(foreground, 158);
        theme.accent = rgba(yellow, 255);
        theme.line = rgb_f(foreground, 0.10);
        theme.hover = rgb_f(accent, 0.55);
        theme.scrim = rgb_f(darker, 0.72);
        theme.mark = rgb_f(yellow, 1.0);
        theme.code_ink = foreground;
        theme.code_wash = rgb_f(foreground, 0.15);
        theme.code_well = rgb_f(foreground, 0.08);
        if let Some((bg, ink)) = highlight_from(&palette) {
            theme.highlight_bg = Some(rgb_f(bg, 1.0));
            theme.highlight_ink = Some(ink);
        }
    }
    apply_shell(&mut theme, &shell);
    theme.stamp = stamp_of(&theme);
    theme
}

/// Sizes and the bar colors follow the shell, including when the palette is missing.
fn apply_shell(theme: &mut Theme, shell: &std::collections::HashMap<String, String>) {
    theme.scale = base_scale(shell);
    theme.body_small = font_token(shell, "body-small", 11.0 / 12.0);
    theme.bar_bg = bar_fill(shell, theme.raise);
    theme.bar_ink = bar_label(shell, theme.ink_soft);
}

/// A shell font token. The written px wins. Otherwise it follows the base size,
/// the same way the shell derives `body-small` (11 at a base of 12).
fn font_token(shell: &std::collections::HashMap<String, String>, key: &str, mult: f32) -> f32 {
    let name = format!("font.{key}");
    if let Some(value) = shell.get(&name).and_then(|value| value.parse::<f32>().ok()) {
        if value > 0.0 {
            return value.round().max(1.0);
        }
    }
    let base = shell
        .get("font.base-size")
        .and_then(|value| value.parse::<f32>().ok())
        .filter(|value| *value >= 1.0)
        .unwrap_or(12.0);
    (base * mult).round().max(1.0)
}

fn bar_fill(shell: &std::collections::HashMap<String, String>, fallback: [f32; 4]) -> [f32; 4] {
    let Some(rgb) = shell
        .get("bar.background")
        .and_then(|value| parse_hex(value))
    else {
        return fallback;
    };
    let alpha = shell
        .get("bar.background-alpha")
        .and_then(|value| value.parse::<f32>().ok())
        .unwrap_or(1.0)
        .clamp(0.0, 1.0);
    rgb_f(rgb, alpha)
}

/// Both colors, or neither. `highlight_*` wins, then `selection_*`.
fn highlight_from(
    palette: &std::collections::HashMap<String, String>,
) -> Option<([u8; 3], [u8; 3])> {
    color_pair(palette, "highlight_background", "highlight_foreground")
        .or_else(|| color_pair(palette, "selection_background", "selection_foreground"))
}

fn color_pair(
    palette: &std::collections::HashMap<String, String>,
    background: &str,
    foreground: &str,
) -> Option<([u8; 3], [u8; 3])> {
    let bg = palette.get(background).and_then(|value| parse_hex(value))?;
    let ink = palette.get(foreground).and_then(|value| parse_hex(value))?;
    Some((bg, ink))
}

fn bar_label(shell: &std::collections::HashMap<String, String>, fallback: [u8; 4]) -> [u8; 4] {
    shell
        .get("bar.text")
        .and_then(|value| parse_hex(value))
        .map(|rgb| rgba(rgb, 255))
        .unwrap_or(fallback)
}

fn base_scale(shell: &std::collections::HashMap<String, String>) -> f32 {
    let base = shell
        .get("font.base-size")
        .and_then(|value| value.parse::<f32>().ok())
        .filter(|value| *value >= 1.0)
        .unwrap_or(12.0);
    (base / 12.0).clamp(0.5, 3.0)
}

fn fallback(ui: &str, mono: &str) -> Theme {
    let foreground = [0xe7, 0xe1, 0xd5];
    Theme {
        bg: crate::layout::BG,
        raise: crate::layout::RAISE,
        ink: crate::layout::INK,
        muted: crate::layout::MUTED,
        ink_soft: crate::layout::INK_SOFT,
        brand: crate::layout::BRAND,
        accent: crate::layout::AMBER,
        line: crate::layout::hex(0xece7dc, 0.10),
        hover: crate::layout::hex(0xe0a45a, 0.55),
        scrim: crate::layout::hex(0x0a0907, 0.72),
        mark: crate::layout::hex(0xe0a45a, 1.0),
        code_ink: foreground,
        code_wash: crate::layout::hex(0xece7dc, 0.15),
        code_well: crate::layout::hex(0xece7dc, 0.08),
        ui_font: ui.to_string(),
        mono_font: mono.to_string(),
        scale: 1.0,
        body_small: 11.0,
        bar_bg: crate::layout::RAISE,
        bar_ink: crate::layout::INK_SOFT,
        highlight_bg: None,
        highlight_ink: None,
        stamp: 0,
    }
}

fn stamp_of(theme: &Theme) -> u32 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    hash = mix(hash, &theme.ink);
    hash = mix(hash, &theme.muted);
    hash = mix(hash, &theme.accent);
    hash = mix(hash, &theme.code_ink);
    hash = mix(hash, &theme.bg[0].to_bits().to_le_bytes());
    hash = mix(hash, &theme.bg[1].to_bits().to_le_bytes());
    hash = mix(hash, &theme.bg[2].to_bits().to_le_bytes());
    hash = mix(hash, theme.ui_font.as_bytes());
    hash = mix(hash, theme.mono_font.as_bytes());
    hash = mix(hash, &theme.scale.to_bits().to_le_bytes());
    hash = mix(hash, &theme.body_small.to_bits().to_le_bytes());
    hash = mix(hash, &theme.bar_ink);
    hash = mix(hash, &theme.bar_bg[0].to_bits().to_le_bytes());
    hash = mix(hash, &theme.bar_bg[1].to_bits().to_le_bytes());
    hash = mix(hash, &theme.bar_bg[2].to_bits().to_le_bytes());
    hash = mix(hash, &theme.bar_bg[3].to_bits().to_le_bytes());
    hash = mix(hash, &theme.highlight_ink.unwrap_or([0, 0, 0]));
    if let Some(bg) = theme.highlight_bg {
        hash = mix(hash, &bg[0].to_bits().to_le_bytes());
        hash = mix(hash, &bg[1].to_bits().to_le_bytes());
        hash = mix(hash, &bg[2].to_bits().to_le_bytes());
    }
    hash as u32
}

fn mix(hash: u64, bytes: &[u8]) -> u64 {
    let mut hash = hash;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100_0000_01b3);
    }
    hash
}

/// Drops a `#` comment, and keeps a `#` that is inside a quoted color.
fn strip_comment(line: &str) -> &str {
    let mut quote = None;
    for (index, ch) in line.char_indices() {
        match (quote, ch) {
            (Some(open), ch) if ch == open => quote = None,
            (None, '"' | '\'') => quote = Some(ch),
            (None, '#') => return line[..index].trim(),
            _ => {}
        }
    }
    line.trim()
}

fn assigns(text: &str) -> std::collections::HashMap<String, String> {
    let mut section = String::new();
    let mut map = std::collections::HashMap::new();
    for line in text.lines() {
        let line = strip_comment(line);
        if line.is_empty() {
            continue;
        }
        if let Some(name) = line
            .strip_prefix('[')
            .and_then(|rest| rest.strip_suffix(']'))
        {
            section = name.trim().to_string();
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if key.is_empty() {
            continue;
        }
        let name = if section.is_empty() {
            key.to_string()
        } else {
            format!("{section}.{key}")
        };
        let value = value
            .trim()
            .trim_matches('"')
            .trim_matches('\'')
            .to_string();
        map.insert(name, value);
    }
    map
}

fn rgb(map: &std::collections::HashMap<String, String>, key: &str, fallback: [u8; 3]) -> [u8; 3] {
    map.get(key)
        .and_then(|value| parse_hex(value))
        .unwrap_or(fallback)
}

fn parse_hex(value: &str) -> Option<[u8; 3]> {
    let hex = value.trim().trim_start_matches('#');
    if hex.len() != 6 || !hex.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return None;
    }
    Some([
        u8::from_str_radix(&hex[0..2], 16).ok()?,
        u8::from_str_radix(&hex[2..4], 16).ok()?,
        u8::from_str_radix(&hex[4..6], 16).ok()?,
    ])
}

fn rgba(rgb: [u8; 3], alpha: u8) -> [u8; 4] {
    [rgb[0], rgb[1], rgb[2], alpha]
}

fn rgb_f(rgb: [u8; 3], alpha: f32) -> [f32; 4] {
    [
        rgb[0] as f32 / 255.0,
        rgb[1] as f32 / 255.0,
        rgb[2] as f32 / 255.0,
        alpha,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    const COLORS: &str = r##"
mode = "dark"
accent = "#7daea3"
background = "#282828"
darker_background = "#161616"
foreground = "#d4be98"
light_foreground = "#bdae93"
bright_foreground = "#d4be98"
yellow = "#d8a657" # headings
"##;

    #[test]
    fn a_theme_sets_the_shelf_and_the_text_size() {
        let theme = theme_from(
            COLORS,
            "[font]\nbase-size = 12\n",
            "[font]\nbase-size = 18\n",
            "JetBrainsMono Nerd Font",
        );
        assert!((theme.scale - 1.5).abs() < 1e-4);
        assert_eq!(theme.ui_font, "JetBrainsMono Nerd Font");
        assert_eq!(theme.mono_font, theme.ui_font);
        assert_eq!(theme.accent, [0xd8, 0xa6, 0x57, 255]);
        assert_eq!(theme.ink, [0xd4, 0xbe, 0x98, 255]);
        assert_eq!(theme.muted, [0xbd, 0xae, 0x93, 255]);
        assert_eq!(theme.code_ink, [0xd4, 0xbe, 0x98]);
        assert!((theme.bg[0] - 0x16 as f32 / 255.0).abs() < 1e-4);
        assert!((theme.raise[0] - 0x28 as f32 / 255.0).abs() < 1e-4);
        assert!((theme.body_small - 17.0).abs() < 0.1);
    }

    #[test]
    fn a_smaller_font_and_the_bar_follow_the_shell() {
        let theme = theme_from(
            COLORS,
            "[font]\nbase-size = 12\nbody-small = 11\n\n[bar]\nbackground = \"#111111\"\nbackground-alpha = 0.5\ntext = \"#eeeeee\"\n",
            "[font]\nbody-small = 13\n",
            "Adwaita Mono",
        );
        assert!((theme.body_small - 13.0).abs() < 0.1);
        assert!((theme.bar_bg[0] - 0x11 as f32 / 255.0).abs() < 1e-4);
        assert!((theme.bar_bg[3] - 0.5).abs() < 1e-4);
        assert_eq!(theme.bar_ink, [0xee, 0xee, 0xee, 255]);
        assert!(theme.highlight_bg.is_none());
    }

    #[test]
    fn a_highlight_pair_colors_the_card_pill() {
        let named = theme_from(
            r##"
            background = "#282828"
            foreground = "#d4be98"
            highlight_background = "#112233"
            highlight_foreground = "#ffeecc"
            "##,
            "",
            "",
            "",
        );
        let (bg, ink) = named.highlight_bg.zip(named.highlight_ink).unwrap();
        assert!((bg[0] - 0x11 as f32 / 255.0).abs() < 1e-4);
        assert!((bg[1] - 0x22 as f32 / 255.0).abs() < 1e-4);
        assert!((bg[2] - 0x33 as f32 / 255.0).abs() < 1e-4);
        assert!((bg[3] - 1.0).abs() < 1e-4);
        assert_eq!(ink, [0xff, 0xee, 0xcc]);

        let selected = theme_from(
            r##"
            background = "#282828"
            foreground = "#d4be98"
            selection_background = "#abcdef"
            selection_foreground = "#101010"
            "##,
            "",
            "",
            "",
        );
        assert_eq!(selected.highlight_ink, Some([0x10, 0x10, 0x10]));

        let both = theme_from(
            r##"
            background = "#282828"
            highlight_background = "#112233"
            highlight_foreground = "#ffeecc"
            selection_background = "#abcdef"
            selection_foreground = "#101010"
            "##,
            "",
            "",
            "",
        );
        assert_eq!(both.highlight_ink, Some([0xff, 0xee, 0xcc]));

        let background_only = theme_from(
            r##"
            background = "#282828"
            foreground = "#d4be98"
            selection = "#504945"
            "##,
            "",
            "",
            "",
        );
        assert!(background_only.highlight_bg.is_none());
        assert!(background_only.highlight_ink.is_none());
    }

    #[test]
    fn a_missing_palette_keeps_the_built_in_shelf() {
        let theme = theme_from("", "", "", "");
        assert_eq!(theme.ink, crate::layout::INK);
        assert!((theme.scale - 1.0).abs() < 1e-4);
        assert_eq!(theme.ui_font, "Adwaita Sans");
        assert_eq!(theme.mono_font, "Adwaita Mono");
    }
}
