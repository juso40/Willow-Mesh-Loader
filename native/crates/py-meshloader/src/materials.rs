//! Original glTF material-slot mapping to loaded engine material paths.

use std::collections::BTreeMap;

pub(crate) type MaterialMapping = BTreeMap<Option<usize>, String>;

pub(crate) fn section_material_paths(
    sections: &[mesh_import::Section],
    fallback: &str,
    mapping: Option<&MaterialMapping>,
) -> Result<Vec<String>, String> {
    if mapping.is_some_and(|mapping| {
        mapping.keys().any(|key| {
            !sections
                .iter()
                .any(|section| section.source_material == *key)
        })
    }) {
        return Err("material mapping contains unused/unknown source slot".into());
    }
    let paths: Vec<_> = sections
        .iter()
        .map(|section| {
            mapping
                .and_then(|mapping| mapping.get(&section.source_material))
                .map_or(fallback, String::as_str)
                .to_owned()
        })
        .collect();
    if paths.iter().any(|path| {
        path.is_empty() || path.len() > mesh_runtime::MAX_OBJECT_PATH_BYTES || path.contains('\0')
    }) {
        return Err("material paths must be nonempty bounded strings without NUL".into());
    }
    Ok(paths)
}
