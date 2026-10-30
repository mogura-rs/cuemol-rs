use base64::{engine::general_purpose::STANDARD, Engine};
use cuemol_core::{CueMolError, Result, Uid};
use cuemol_desktop_core::{DesktopAppState, GuiClipboardPayload, GuiPastedNode};
use serde_json::json;

pub const MAGIC: &str = "CueMolClipboard/1";

#[derive(Debug, Clone)]
pub enum Target {
    Object(Uid),
    Renderer { object: Uid, renderer: Uid },
    Renderers(Vec<(Uid, Uid)>),
    Camera { scene: Uid, name: String },
    Style { scene: Uid, index: usize },
}

pub fn renderer_target(state: &DesktopAppState, renderer: Uid) -> Result<Target> {
    Ok(Target::Renderer {
        object: crate::commands::renderer_owner(state, renderer)?,
        renderer,
    })
}

pub fn copy_payload(state: &DesktopAppState, target: &Target) -> Result<GuiClipboardPayload> {
    Ok(match target {
        Target::Object(id) => state.copy_workspace_object(*id)?.to_payload()?,
        Target::Renderer { object, renderer } => state
            .copy_workspace_renderer(*object, *renderer)?
            .to_payload()?,
        Target::Renderers(targets) => state.copy_workspace_renderers(targets)?.to_payload()?,
        Target::Camera { scene, name } => {
            state.copy_workspace_camera(*scene, name)?.to_payload()?
        }
        Target::Style { scene, index } => {
            state.copy_workspace_style(*scene, *index)?.to_payload()?
        }
    })
}

pub fn copy(state: &DesktopAppState, target: &Target) -> Result<String> {
    Ok(encode(&copy_payload(state, target)?))
}

pub fn encode(payload: &GuiClipboardPayload) -> String {
    encode_bytes(payload.kind, payload.form, &payload.bytes)
}

fn encode_bytes(kind: &str, form: &str, bytes: &[u8]) -> String {
    format!(
        "{MAGIC}\n{}\n{}\n",
        json!({"kind":kind,"form":form}),
        STANDARD.encode(bytes)
    )
}

#[derive(Debug)]
pub struct Payload {
    pub kind: String,
    pub form: String,
    pub bytes: Vec<u8>,
}

impl Payload {
    pub fn encode(&self) -> String {
        encode_bytes(&self.kind, &self.form, &self.bytes)
    }
}

pub fn header(text: &str) -> Result<(&'static str, &'static str)> {
    let mut lines = text.splitn(3, '\n');
    if lines
        .next()
        .unwrap_or("")
        .trim_start_matches('\u{feff}')
        .trim()
        != MAGIC
    {
        return Err(CueMolError::invalid_argument(
            "The clipboard does not contain CueMol data",
        ));
    }
    let metadata: serde_json::Value = serde_json::from_str(lines.next().unwrap_or("").trim())
        .map_err(|_| CueMolError::parse("invalid CueMol clipboard header"))?;
    let kind = match metadata["kind"].as_str() {
        Some("object") => "object",
        Some("renderer") => "renderer",
        Some("camera") => "camera",
        Some("style") => "style",
        Some("paint") => "paint",
        _ => return Err(CueMolError::parse("unknown CueMol clipboard kind")),
    };
    let form = if metadata["form"] == "rendArray" {
        "rendArray"
    } else {
        "single"
    };
    if lines.next().is_none() {
        return Err(CueMolError::parse("missing CueMol clipboard data"));
    }
    Ok((kind, form))
}

pub fn decode(text: &str) -> Result<Payload> {
    let (kind, form) = header(text)?;
    let data = text.splitn(3, '\n').nth(2).unwrap();
    let data: String = data
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    let bytes = STANDARD
        .decode(data)
        .map_err(|_| CueMolError::parse("invalid CueMol clipboard data"))?;
    Ok(Payload {
        kind: kind.to_string(),
        form: form.to_string(),
        bytes,
    })
}

pub const LEGACY_FORMATS: [(&str, &str, &str); 6] = [
    ("application/x-cuemol2-scenexml-rend", "renderer", "single"),
    (
        "application/x-cuemol2-scenexml-rend-array",
        "renderer",
        "rendArray",
    ),
    ("application/x-cuemol2-scenexml-obj", "object", "single"),
    ("application/x-cuemol2-scenexml-cam", "camera", "single"),
    ("application/x-cuemol2-scenexml-style", "style", "single"),
    ("application/x-cuemol2-json-paint", "paint", "single"),
];

pub fn legacy_bytes(payload: &GuiClipboardPayload, windows: bool) -> Vec<u8> {
    let mut bytes = if payload.kind == "paint" {
        String::from_utf8_lossy(&payload.bytes)
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>()
    } else {
        payload
            .bytes
            .iter()
            .flat_map(|byte| u16::from(*byte).to_le_bytes())
            .collect()
    };
    if windows {
        bytes.extend_from_slice(&[0, 0]);
    }
    bytes
}

pub fn from_legacy(kind: &str, form: &str, bytes: &[u8]) -> Payload {
    let mut units: Vec<_> = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect();
    while units.last() == Some(&0) {
        units.pop();
    }
    Payload {
        kind: kind.to_string(),
        form: form.to_string(),
        bytes: if kind == "paint" {
            String::from_utf16_lossy(&units).into_bytes()
        } else {
            units.into_iter().map(|unit| unit as u8).collect()
        },
    }
}

pub fn paste(
    state: &mut DesktopAppState,
    text: &str,
    target: Option<Uid>,
    target_type: Option<&str>,
) -> Result<GuiPastedNode> {
    let payload = decode(text)?;
    state.paste_clipboard_bytes(
        &payload.kind,
        &payload.form,
        &payload.bytes,
        target,
        target_type,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renderer_target_uses_its_owner_instead_of_the_current_molecule() {
        let mut state = DesktopAppState::new();
        let scene = state.ensure_active_scene();
        let owner = state
            .scene_manager_mut()
            .scene_mut(scene)
            .unwrap()
            .add_molecule("owner", cuemol_molstr::MolCoord::new());
        let other = state
            .scene_manager_mut()
            .scene_mut(scene)
            .unwrap()
            .add_molecule("other", cuemol_molstr::MolCoord::new());
        let renderer = state
            .scene_manager_mut()
            .object_mut(owner)
            .unwrap()
            .add_renderer("simple", "lines");
        let target = renderer_target(&state, renderer).unwrap();
        assert!(
            matches!(target, Target::Renderer { object, .. } if object == owner && object != other)
        );
        assert!(copy_payload(&state, &target).is_ok());
        assert!(renderer_target(&state, other).is_err());
    }
    #[test]
    fn envelope_retains_binary_payload_and_rejects_truncation() {
        let payload = GuiClipboardPayload {
            kind: "object",
            form: "single",
            bytes: (0..=255).collect(),
        };
        let envelope = encode(&payload);
        let parsed = decode(&format!("\u{feff}{}", envelope.replace('\n', "\r\n"))).unwrap();
        assert_eq!(parsed.bytes, payload.bytes);
        assert_eq!(parsed.kind, "object");
        assert_eq!(parsed.form, "single");
        assert!(decode("ordinary text").is_err());
        assert!(decode("CueMolClipboard/1\n{\"kind\":\"object\"}\nYWJ").is_err());
        assert!(decode("CueMolClipboard/1\n{\"kind\":\"scene\"}\nYWJj").is_err());
    }

    #[test]
    fn legacy_formats_preserve_binary_xml_and_unicode_paint() {
        for (kind, bytes) in [
            ("object", (0..=255).collect()),
            (
                "paint",
                "[{\"sel\":\"chain 日本\",\"col\":\"red\"}]"
                    .as_bytes()
                    .to_vec(),
            ),
        ] {
            let payload = GuiClipboardPayload {
                kind,
                form: "single",
                bytes,
            };
            for windows in [false, true] {
                let mut wire = legacy_bytes(&payload, windows);
                wire.extend_from_slice(&[0, 0, 0, 0, 0]);
                assert_eq!(from_legacy(kind, "single", &wire).bytes, payload.bytes);
            }
        }
    }

    #[test]
    fn header_probe_does_not_decode_the_body() {
        assert_eq!(
            header("CueMolClipboard/1\n{\"kind\":\"renderer\",\"form\":\"rendArray\"}\nnot base64")
                .unwrap(),
            ("renderer", "rendArray")
        );
        assert!(header("CueMolClipboard/1\n{}").is_err());
    }
}
