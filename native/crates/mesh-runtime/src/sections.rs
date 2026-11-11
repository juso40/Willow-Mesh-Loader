//! Checked section partitions and POD section bytes. No engine calls or reads.
use crate::{Error, Result, encoding::TRIANGLE_INDICES};
use serde::Serialize;

/// Maximum count of section or material records in a signed native array.
/// Skeletal section material ordinals are additionally bounded by u16.
pub const MAX_SECTIONS: usize = i32::MAX as usize;
/// Requested nonempty consecutive subrange of a triangle-list index buffer.
#[derive(Clone, Copy, Debug)]
pub struct Section {
    /// Offset in indices, not bytes.
    pub first_index: usize,
    /// Triangle count in this range.
    pub triangles: usize,
    /// Index into the caller's material-path list.
    pub material_index: usize,
}
/// Fully checked metadata. Min/max are derived from actual indices, not guessed.
#[derive(Debug, Clone, Serialize)]
pub struct CheckedSection {
    /// Offset in index elements.
    pub first_index: u32,
    /// Primitive count.
    pub triangles: u32,
    /// Minimum referenced vertex in this range.
    pub min_vertex_index: u32,
    /// Maximum referenced vertex in this range.
    pub max_vertex_index: u32,
    /// Component override/material slot.
    pub material_index: u32,
}
/// Validates an exact, ordered, gap/overlap-free partition. All validation precedes
/// engine mutation. Low-level encoding separately checks finite geometry/basis.
pub fn validate(
    indices: &[u32],
    vertices: usize,
    sections: &[Section],
    materials: usize,
) -> Result<Vec<CheckedSection>> {
    if sections.is_empty()
        || sections.len() > MAX_SECTIONS
        || materials == 0
        || materials > MAX_SECTIONS
    {
        return Err(Error(
            "section/material count exceeds the signed native array range".into(),
        ));
    }
    let mut end = 0;
    let mut out = Vec::with_capacity(sections.len());
    for section in sections {
        if section.first_index != end
            || section.triangles == 0
            || section.material_index >= materials
        {
            return Err(Error("invalid section partition/material slot".into()));
        }
        let next = section
            .triangles
            .checked_mul(TRIANGLE_INDICES)
            .and_then(|n| end.checked_add(n))
            .ok_or_else(|| Error("section range overflow".into()))?;
        let range = indices
            .get(end..next)
            .ok_or_else(|| Error("section exceeds index buffer".into()))?;
        let min = *range
            .iter()
            .min()
            .ok_or_else(|| Error("empty section".into()))?;
        let max = *range
            .iter()
            .max()
            .ok_or_else(|| Error("empty section".into()))?;
        if max as usize >= vertices || next > i32::MAX as usize {
            return Err(Error(
                "section vertex/index range exceeds native policy".into(),
            ));
        }
        out.push(CheckedSection {
            first_index: end as u32,
            triangles: section.triangles as u32,
            min_vertex_index: min,
            max_vertex_index: max,
            material_index: section.material_index as u32,
        });
        end = next;
    }
    if end != indices.len() {
        return Err(Error("sections do not cover complete index buffer".into()));
    }
    Ok(out)
}
/// Encodes known empty/default 0x40 section records. Addresses here are private
/// staging words supplied by the backend after SDK material lookup, not a Python
/// address API. Empty FString/fragments/platform, no collision, cast shadow.
pub(crate) fn encode(sections: &[CheckedSection], materials: &[u32]) -> Result<Vec<u8>> {
    const RECORD_SIZE: usize = 0x40;
    const CAST_SHADOW: usize = 0x18;
    const FIRST_INDEX: usize = 0x1c;
    const TRIANGLES: usize = 0x20;
    const MIN_VERTEX: usize = 0x24;
    const MAX_VERTEX: usize = 0x28;
    const MATERIAL_SLOT: usize = 0x2c;
    let mut bytes = vec![0; sections.len() * RECORD_SIZE];
    for (i, s) in sections.iter().enumerate() {
        let material = *materials
            .get(s.material_index as usize)
            .ok_or_else(|| Error("missing material reference".into()))?;
        for (offset, value) in [
            (0, material),
            (CAST_SHADOW, 1),
            (FIRST_INDEX, s.first_index),
            (TRIANGLES, s.triangles),
            (MIN_VERTEX, s.min_vertex_index),
            (MAX_VERTEX, s.max_vertex_index),
            (MATERIAL_SLOT, s.material_index),
        ] {
            bytes[i * RECORD_SIZE + offset..i * RECORD_SIZE + offset + size_of::<u32>()]
                .copy_from_slice(&value.to_le_bytes());
        }
    }
    Ok(bytes)
}
