use std::fs;
use std::path::{Path, PathBuf};

pub struct Saver {
    pub id: String,
    pub name: String,
    pub blurb: String,
    /// Shadertoy image shader, as written. The renderer wraps this before compiling.
    pub source: String,
    /// Where it came from, for the message printed when it does not compile.
    pub origin: String,
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
    Ok(Saver {
        id: id.to_string(),
        name,
        blurb,
        source: source.to_string(),
        origin,
    })
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
pub fn fragment_source(user: &str) -> String {
    const PREAMBLE: &str = r#"#version 450
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
};

#define iGlobalTime iTime
#define texture2D texture
#define texture2DLod textureLod
#define textureCube texture

#line 1
"#;
    const EPILOGUE: &str = r#"
void main() {
    vec4 color = vec4(0.0, 0.0, 0.0, 1.0);
    vec2 fragCoord = vec2(gl_FragCoord.x, iResolution.y - gl_FragCoord.y);
    mainImage(color, fragCoord);
    shadertoy_out = color;
}
"#;
    let mut out = String::with_capacity(PREAMBLE.len() + user.len() + EPILOGUE.len() + 1);
    out.push_str(PREAMBLE);
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
    }
}
