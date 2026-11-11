//! Stable grouping of triangle ranges by original glTF material slots.

use crate::{MAX_SECTIONS, Result, Section, fail};

pub(crate) fn group(indices: &[u32], sections: &[Section]) -> Result<(Vec<u32>, Vec<Section>)> {
    let (indices, sections, _) = group_with_offsets(indices, sections)?;
    Ok((indices, sections))
}

pub(crate) fn group_with_offsets(
    indices: &[u32],
    sections: &[Section],
) -> Result<(Vec<u32>, Vec<Section>, Vec<usize>)> {
    let mut slots = Vec::new();
    let mut groups: Vec<Vec<u32>> = Vec::new();
    for section in sections {
        let slot = if let Some(i) = slots.iter().position(|s| *s == section.source_material) {
            i
        } else {
            if slots.len() == MAX_SECTIONS {
                return Err(fail(
                    "source material sections exceed the signed native array range",
                ));
            }
            slots.push(section.source_material);
            groups.push(Vec::new());
            slots.len() - 1
        };
        groups[slot].extend_from_slice(
            &indices[section.first_index..section.first_index + section.triangles * 3],
        );
    }
    let mut grouped_indices = Vec::with_capacity(indices.len());
    let mut grouped_sections = Vec::with_capacity(slots.len());
    for (source_material, group) in slots.into_iter().zip(groups) {
        grouped_sections.push(Section {
            first_index: grouped_indices.len(),
            triangles: group.len() / 3,
            source_material,
        });
        grouped_indices.extend(group);
    }
    let mut cursors: Vec<_> = grouped_sections.iter().map(|s| s.first_index).collect();
    let mut offsets = Vec::with_capacity(sections.len());
    for section in sections {
        let slot = grouped_sections
            .iter()
            .position(|s| s.source_material == section.source_material)
            .ok_or_else(|| fail("missing grouped material"))?;
        offsets.push(cursors[slot]);
        cursors[slot] += section.triangles * 3;
    }
    Ok((grouped_indices, grouped_sections, offsets))
}
