//! Material placeholders and extension admission for selected source records.

use std::collections::BTreeMap;

use serde_json::{Value, json};

use crate::{Error, Result, fail};

pub(crate) struct Extensions(BTreeMap<String, Vec<String>>);

impl Extensions {
    pub(crate) fn check(&self, path: &str) -> Result<()> {
        if let Some(names) = self.0.get(path) {
            return Err(fail(&format!(
                "unsupported extensions on selected {path}: {}",
                names.join(", ")
            )));
        }
        Ok(())
    }
}

fn collect_extensions(value: &Value, path: &str, out: &mut BTreeMap<String, Vec<String>>) {
    match value {
        Value::Object(object) => {
            if let Some(Value::Object(extensions)) = object.get("extensions") {
                let names: Vec<_> = extensions
                    .keys()
                    .filter(|name| !name.starts_with("KHR_materials_"))
                    .cloned()
                    .collect();
                if !names.is_empty() {
                    out.insert(path.into(), names);
                }
            }
            for (key, value) in object {
                if key != "extensions" && key != "extras" {
                    collect_extensions(value, &format!("{path}/{key}"), out);
                }
            }
        }
        Value::Array(array) => {
            for (index, value) in array.iter().enumerate() {
                collect_extensions(value, &format!("{path}/{index}"), out);
            }
        }
        _ => {}
    }
}

pub(crate) fn parse(bytes: &[u8]) -> Result<(gltf::Gltf, Extensions)> {
    let glb = if bytes.starts_with(b"glTF") {
        Some(gltf::binary::Glb::from_slice(bytes).map_err(|e| Error(format!("invalid GLB: {e}")))?)
    } else {
        None
    };
    let mut root: Value =
        serde_json::from_slice(glb.as_ref().map_or(bytes, |glb| glb.json.as_ref()))
            .map_err(|e| Error(format!("invalid glTF JSON: {e}")))?;
    let object = root
        .as_object_mut()
        .ok_or_else(|| fail("glTF must be an object"))?;
    // Material identities are slot ordinals only. No shader, texture or image data
    // enters the typed glTF document, including exporter-specific material extensions.
    if let Some(materials) = object.get_mut("materials") {
        let materials = materials
            .as_array_mut()
            .ok_or_else(|| fail("materials must be an array"))?;
        materials.fill(json!({}));
    }
    for key in [
        "textures",
        "images",
        "samplers",
        "extensionsUsed",
        "extensionsRequired",
    ] {
        object.remove(key);
    }
    let mut extensions = BTreeMap::new();
    collect_extensions(&root, "", &mut extensions);
    let json = serde_json::from_value(root).map_err(|e| Error(format!("invalid glTF: {e}")))?;
    let document =
        gltf::Document::from_json(json).map_err(|e| Error(format!("invalid glTF: {e}")))?;
    Ok((
        gltf::Gltf {
            document,
            blob: glb.and_then(|glb| glb.bin.map(|bin| bin.into_owned())),
        },
        Extensions(extensions),
    ))
}
