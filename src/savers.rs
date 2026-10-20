use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Detail when a saver has not been set. Half the window's pixels.
pub const DEFAULT_QUALITY: u8 = 50;

/// Side length of the noise picture used when a shader asks for a channel.
/// 256 is the size Clouds and the same family divide by. Green is red
/// shifted by (37, 17), and alpha is blue shifted the same way.
pub const STAND_IN: u32 = 256;

/// What a Shadertoy channel was asking for. The app still runs the shader.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ChannelKind {
    /// A flat picture: an image, a buffer, the keyboard, or sound.
    Image,
    Cube,
}

/// Why a shader is on the shelf but not running. The file on disk is left as saved.
pub struct Hold {
    /// One or two sentences on the card, in place of the blurb while the card is highlighted.
    pub card: String,
    /// The still note. Paragraphs are separated by a blank line.
    pub page: String,
}

pub struct Saver {
    pub id: String,
    pub name: String,
    pub blurb: String,
    /// Shadertoy image shader, as written. The renderer wraps this before compiling.
    pub source: String,
    /// Where it came from, for the message printed when it does not compile.
    pub origin: String,
    /// Shelf note for what a channel will not receive. Empty when nothing is missing.
    pub note: String,
    /// Set when this app could not compile the shader. The card stays on the shelf.
    pub hold: Option<Hold>,
}

pub fn shader_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("SAVETHESCREEN_SHADERS") {
        if !dir.is_empty() {
            return PathBuf::from(dir);
        }
    }
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .unwrap_or_else(|| PathBuf::from(".config"));
    base.join("savethescreen").join("shaders")
}

/// Remembered detail, beside the shader folder. Each line is `percent id`.
pub fn quality_path() -> PathBuf {
    shader_dir()
        .parent()
        .map(|dir| dir.join("quality"))
        .unwrap_or_else(|| PathBuf::from("quality"))
}

pub fn snap_quality(percent: u8) -> u8 {
    let stepped = (u16::from(percent) + 5) / 10 * 10;
    stepped.clamp(10, 100) as u8
}

pub fn load_quality(path: &Path) -> HashMap<String, u8> {
    let Ok(text) = fs::read_to_string(path) else {
        return HashMap::new();
    };
    let mut levels = HashMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split_whitespace();
        let Some(percent) = parts.next().and_then(|token| token.parse::<u8>().ok()) else {
            continue;
        };
        let id = parts.collect::<Vec<_>>().join(" ");
        if id.is_empty() {
            continue;
        }
        levels.insert(id, snap_quality(percent));
    }
    levels
}

pub fn save_quality(path: &Path, levels: &HashMap<String, u8>) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    let mut ids: Vec<_> = levels.keys().collect();
    ids.sort();
    let mut text = String::new();
    for id in ids {
        let Some(percent) = levels.get(id) else {
            continue;
        };
        text.push_str(&percent.to_string());
        text.push(' ');
        text.push_str(id);
        text.push('\n');
    }
    fs::write(path, text)
}

/// Built-in savers, plus any `.frag` files in `shader_dir`. A file whose name
/// matches a built-in id replaces that saver. The list is sorted by card title.
pub fn load() -> Vec<Saver> {
    let dir = shader_dir();
    if let Err(err) = fs::create_dir_all(&dir) {
        eprintln!("savethescreen: shader folder {}: {err}", dir.display());
    } else {
        eprintln!("savethescreen: drop a shader in {}", dir.display());
    }
    load_from(&dir)
}

pub(crate) fn load_from(dir: &Path) -> Vec<Saver> {
    let mut savers = builtins();
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) => {
            eprintln!("savethescreen: {}: {err}", dir.display());
            sort_savers(&mut savers);
            return savers;
        }
    };
    let mut paths: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| id_from_path(path).is_some())
        .collect();
    paths.sort();

    let mut added = Vec::new();
    for path in paths {
        let Some(id) = id_from_path(&path) else {
            continue;
        };
        let source = match fs::read_to_string(&path) {
            Ok(source) => source,
            Err(err) => {
                eprintln!("savethescreen: skipped {}: {err}", path.display());
                continue;
            }
        };
        let saver = match parse_shader(&id, &source, path.display().to_string()) {
            Ok(saver) => saver,
            Err(err) => {
                eprintln!("savethescreen: skipped {err}");
                continue;
            }
        };
        if let Some(existing) = savers.iter_mut().find(|item| item.id == saver.id) {
            *existing = saver;
        } else {
            added.push(saver);
        }
    }
    savers.extend(added);
    sort_savers(&mut savers);
    savers
}

fn sort_savers(savers: &mut [Saver]) {
    savers.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.id.cmp(&b.id))
    });
}

fn builtins() -> Vec<Saver> {
    [
        ("veil", include_str!("../shaders/veil.frag")),
        ("drift", include_str!("../shaders/drift.frag")),
        ("ember", include_str!("../shaders/ember.frag")),
        ("bodies", include_str!("../shaders/bodies.frag")),
        ("shoal", include_str!("../shaders/shoal.frag")),
        ("gloam", include_str!("../shaders/gloam.frag")),
        ("cinder", include_str!("../shaders/cinder.frag")),
        ("murk", include_str!("../shaders/murk.frag")),
        ("wisp", include_str!("../shaders/wisp.frag")),
        ("silt", include_str!("../shaders/silt.frag")),
    ]
    .into_iter()
    .map(|(id, source)| {
        parse_shader(id, source, format!("built-in {id}")).expect("built-in shader")
    })
    .collect()
}

fn id_from_path(path: &Path) -> Option<String> {
    let name = path.file_name()?.to_str()?;
    if name.starts_with('.') {
        return None;
    }
    let lower = name.to_ascii_lowercase();
    let stem = lower.strip_suffix(".frag")?;
    if stem.is_empty() {
        return None;
    }
    Some(stem.to_string())
}

fn parse_shader(id: &str, source: &str, origin: String) -> Result<Saver, String> {
    let source = source.strip_prefix('\u{feff}').unwrap_or(source);
    if !source.contains("mainImage") {
        return Err(format!(
            "{origin}: a Shadertoy image shader needs mainImage"
        ));
    }
    let (name, blurb) = card(source, id);
    let note = limit_note(&channel_kinds(source));
    Ok(Saver {
        id: id.to_string(),
        name,
        blurb,
        source: source.to_string(),
        origin,
        note,
        hold: None,
    })
}

/// `iChannel0` through `iChannel3` used in the shader, ignoring comments.
pub fn channel_kinds(source: &str) -> [Option<ChannelKind>; 4] {
    let code = strip_comments(source);
    let mut kinds = [None; 4];
    for index in 0..4 {
        let name = format!("iChannel{index}");
        if !contains_ident(&code, &name) {
            continue;
        }
        kinds[index] = Some(if sampled_as_cube(&code, &name) {
            ChannelKind::Cube
        } else {
            ChannelKind::Image
        });
    }
    kinds
}

/// One or two sentences for the shelf card. Empty when every input is real.
pub fn limit_note(kinds: &[Option<ChannelKind>; 4]) -> String {
    let mut images = Vec::new();
    let mut cubes = Vec::new();
    for (index, kind) in kinds.iter().enumerate() {
        match kind {
            Some(ChannelKind::Image) => images.push(index),
            Some(ChannelKind::Cube) => cubes.push(index),
            None => {}
        }
    }
    let mut sentences = Vec::new();
    if !images.is_empty() {
        sentences.push(format!(
            "{} a 256 noise picture instead of the picture, buffer, keyboard, or sound it asked for.",
            channel_subject(&images)
        ));
    }
    if !cubes.is_empty() {
        sentences.push(format!(
            "{} a stand-in instead of a cubemap.",
            channel_subject(&cubes)
        ));
    }
    sentences.join(" ")
}

fn channel_subject(indices: &[usize]) -> String {
    let names: Vec<_> = indices.iter().map(|index| index.to_string()).collect();
    match names.as_slice() {
        [one] => format!("Channel {one} reads"),
        [first, second] => format!("Channels {first} and {second} read"),
        many => {
            let last = many.last().unwrap();
            let head = many[..many.len() - 1].join(", ");
            format!("Channels {head}, and {last} read")
        }
    }
}

fn contains_ident(code: &str, name: &str) -> bool {
    code.match_indices(name).any(|(at, _)| {
        let before = code[..at].chars().next_back();
        let after = code[at + name.len()..].chars().next();
        let ident = |c: char| c == '_' || c.is_ascii_alphanumeric();
        !before.is_some_and(ident) && !after.is_some_and(ident)
    })
}

fn sampled_as_cube(code: &str, name: &str) -> bool {
    let compact: String = code.chars().filter(|c| !c.is_whitespace()).collect();
    [
        format!("texture({name},vec3"),
        format!("textureLod({name},vec3"),
        format!("textureGrad({name},vec3"),
    ]
    .iter()
    .any(|needle| compact.contains(needle))
}

fn strip_comments(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut chars = source.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '/' && chars.peek() == Some(&'/') {
            chars.next();
            for next in chars.by_ref() {
                if next == '\n' {
                    out.push('\n');
                    break;
                }
            }
        } else if c == '/' && chars.peek() == Some(&'*') {
            chars.next();
            while let Some(next) = chars.next() {
                if next == '*' && chars.peek() == Some(&'/') {
                    chars.next();
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Leading `// name:` and `// blurb:` lines. A Shadertoy paste with none of
/// these still loads: the card title comes from the file name.
fn card(source: &str, id: &str) -> (String, String) {
    let mut name = None;
    let mut blurb = None;
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Some(rest) = trimmed.strip_prefix("//") else {
            break;
        };
        let Some((key, value)) = rest.trim().split_once(':') else {
            continue;
        };
        match key.trim() {
            "name" => {
                let value = value.trim();
                if !value.is_empty() {
                    name = Some(value.to_string());
                }
            }
            "blurb" => blurb = Some(value.trim().to_string()),
            _ => {}
        }
    }
    (
        name.unwrap_or_else(|| title_from_id(id)),
        blurb.unwrap_or_default(),
    )
}

fn title_from_id(id: &str) -> String {
    let mut words = Vec::new();
    for word in id.split(|c: char| c == '-' || c == '_' || c == ' ') {
        if word.is_empty() {
            continue;
        }
        let mut chars = word.chars();
        let Some(first) = chars.next() else {
            continue;
        };
        let mut titled = first.to_uppercase().collect::<String>();
        titled.push_str(chars.as_str());
        words.push(titled);
    }
    if words.is_empty() {
        id.to_string()
    } else {
        words.join(" ")
    }
}

/// Vulkan GLSL around a Shadertoy image shader. `fragCoord` is bottom-left, matching Shadertoy.
/// Naga packs `iTime` into the tail of the `iResolution` vec3. `Globals` in the renderer follows that.
/// A channel the shader asks for becomes a `texture2D` or `textureCube` plus one shared sampler.
/// Those uniforms are declared before `#define textureCube`, which would otherwise rewrite them.
pub fn fragment_source(user: &str) -> String {
    let kinds = channel_kinds(user);
    let mut preamble = String::from(
        r#"#version 450
layout(location = 0) out vec4 shadertoy_out;

layout(std140, set = 0, binding = 0) uniform Shadertoy {
    vec3 iResolution;
    float iTime;
    float iTimeDelta;
    float iFrameRate;
    int iFrame;
    vec4 iMouse;
    vec4 iDate;
    float iSampleRate;
    float iChannelTime[4];
    vec3 iChannelResolution[4];
};

"#,
    );
    if kinds.iter().any(|kind| kind.is_some()) {
        preamble.push_str("layout(set = 0, binding = 1) uniform sampler iChannelSampler;\n");
    }
    for (index, kind) in kinds.iter().enumerate() {
        let Some(kind) = kind else {
            continue;
        };
        let texture = match kind {
            ChannelKind::Image => "texture2D",
            ChannelKind::Cube => "textureCube",
        };
        let sampler = match kind {
            ChannelKind::Image => "sampler2D",
            ChannelKind::Cube => "samplerCube",
        };
        preamble.push_str(&format!(
            "layout(set = 0, binding = {}) uniform {texture} iChannel{index}Tex;\n",
            2 + index
        ));
        preamble.push_str(&format!(
            "#define iChannel{index} {sampler}(iChannel{index}Tex, iChannelSampler)\n"
        ));
    }
    preamble.push_str(
        r#"
#define iGlobalTime iTime
#define texture2D texture
#define texture2DLod textureLod
#define textureCube texture

#line 1
"#,
    );
    const EPILOGUE: &str = r#"
void main() {
    vec4 color = vec4(0.0, 0.0, 0.0, 1.0);
    vec2 fragCoord = vec2(gl_FragCoord.x, iResolution.y - gl_FragCoord.y);
    mainImage(color, fragCoord);
    shadertoy_out = color;
}
"#;
    let mut out = String::with_capacity(preamble.len() + user.len() + EPILOGUE.len() + 1);
    out.push_str(&preamble);
    for line in user.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("#version") || trimmed.starts_with("precision ") {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out.push_str(EPILOGUE);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folder_adds_and_replaces() {
        let dir = std::env::temp_dir().join(format!("savethescreen-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("veil.frag"),
            "// name: Custom Veil\n// night: true\n// blurb: Replaced.\nvoid mainImage(out vec4 fragColor, in vec2 fragCoord) {}\n",
        )
        .unwrap();
        fs::write(
            dir.join("harbor-lights.frag"),
            "void mainImage(out vec4 fragColor, in vec2 fragCoord) {}\n",
        )
        .unwrap();
        fs::write(dir.join("notes.txt"), "not a shader\n").unwrap();
        fs::write(dir.join("broken.frag"), "void main() {}\n").unwrap();

        let savers = load_from(&dir);
        let veil = savers.iter().find(|saver| saver.id == "veil").unwrap();
        assert_eq!(veil.name, "Custom Veil");
        assert_eq!(veil.blurb, "Replaced.");

        let harbor = savers
            .iter()
            .find(|saver| saver.id == "harbor-lights")
            .unwrap();
        assert_eq!(harbor.name, "Harbor Lights");
        assert!(harbor.blurb.is_empty());
        let names: Vec<_> = savers
            .iter()
            .map(|saver| saver.name.to_lowercase())
            .collect();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted);
        assert!(savers.iter().all(|saver| saver.id != "broken"));
        assert!(savers.iter().all(|saver| saver.id != "notes"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn wrapper_keeps_shadertoy_names() {
        let source = fragment_source(
            "#version 300 es\nprecision highp float;\nvoid mainImage(out vec4 fragColor, in vec2 fragCoord) {\n    fragColor = vec4(fragCoord / iResolution.xy, 0.0, 1.0);\n}\n",
        );
        assert!(source.contains("float iTime;"));
        assert!(source.contains("vec4 iMouse;"));
        assert!(source.contains("mainImage(color, fragCoord);"));
        assert!(!source.contains("#version 300 es"));
        assert!(!source.contains("precision highp float;"));
        assert!(source.starts_with("#version 450"));
        assert!(!source.contains("iChannelSampler"));
        assert!(source.contains("float iChannelTime[4];"));
    }

    #[test]
    fn missing_channel_is_named_and_stubbed() {
        let source = "void mainImage(out vec4 fragColor, in vec2 fragCoord) {\n    float radius = texture(iChannel0, vec2(0.2)).x;\n    fragColor = vec4(radius);\n}\n";
        let kinds = channel_kinds(source);
        assert_eq!(
            limit_note(&kinds),
            "Channel 0 reads a 256 noise picture instead of the picture, buffer, keyboard, or sound it asked for."
        );
        let wrapped = fragment_source(source);
        assert!(wrapped.contains("uniform texture2D iChannel0Tex;"));
        assert!(wrapped.contains("#define iChannel0 sampler2D(iChannel0Tex, iChannelSampler)"));
        let define = wrapped.find("#define textureCube").unwrap();
        let texture = wrapped.find("uniform texture2D iChannel0Tex").unwrap();
        assert!(texture < define);

        let cube = "void mainImage(out vec4 fragColor, in vec2 fragCoord) {\n    fragColor = texture(iChannel1, vec3(0.0, 1.0, 0.0));\n}\n";
        assert_eq!(
            limit_note(&channel_kinds(cube)),
            "Channel 1 reads a stand-in instead of a cubemap."
        );

        let both = "void mainImage(out vec4 fragColor, in vec2 fragCoord) {\n    fragColor = texture(iChannel0, vec2(0.0)) + texture(iChannel2, vec3(0.0));\n}\n";
        assert_eq!(
            limit_note(&channel_kinds(both)),
            "Channel 0 reads a 256 noise picture instead of the picture, buffer, keyboard, or sound it asked for. Channel 2 reads a stand-in instead of a cubemap."
        );

        let comment = "// uses iChannel0\nvoid mainImage(out vec4 fragColor, in vec2 fragCoord) { fragColor = vec4(1.0); }\n";
        assert!(limit_note(&channel_kinds(comment)).is_empty());
    }

    #[test]
    fn quality_round_trip() {
        assert_eq!(snap_quality(0), 10);
        assert_eq!(snap_quality(50), 50);
        assert_eq!(snap_quality(100), 100);
        assert_eq!(snap_quality(14), 10);
        assert_eq!(snap_quality(15), 20);

        let path =
            std::env::temp_dir().join(format!("savethescreen-quality-{}", std::process::id()));
        let _ = fs::remove_file(&path);
        let mut levels = HashMap::new();
        levels.insert("wormhood".to_string(), 20);
        levels.insert("harbor lights".to_string(), 100);
        save_quality(&path, &levels).unwrap();
        let loaded = load_quality(&path);
        assert_eq!(loaded.get("wormhood"), Some(&20));
        assert_eq!(loaded.get("harbor lights"), Some(&100));
        let _ = fs::remove_file(&path);
    }
}
