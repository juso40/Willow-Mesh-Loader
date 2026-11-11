//! Host-side staging of BL2 skeletal native records. No engine calls.
//!
//! Produces element bytes and scalar fields for palette-bounded GPU chunks
//! and material sections using the paired
//! BL2/TPS layouts encoded below. TArray
//! headers contain live pointers and are stamped by the native backend
//! at attach time. This module never fabricates them. FName words must come
//! from the SDK's FName initialization path at runtime.

use crate::{
    Error, Result,
    encoding::{PACKED_NORMAL_BYTES, PACKED_NORMAL_MIDPOINT, TRIANGLE_INDICES, unpack_normal},
    sections::CheckedSection,
    skeletal::{ReferenceBone, SkinnedMesh, skeleton},
};

/// BL2 `FSkelMeshSection` record size.
pub const SECTION_SIZE: usize = 0x10;
/// BL2 `FSkelMeshChunk` record size.
pub const CHUNK_SIZE: usize = 0x34;
/// BL2 `FMeshBone` record size.
pub const BONE_SIZE: usize = 0x50;
/// BL2 `FSkeletalMeshLODInfo` record size.
pub const LOD_INFO_SIZE: usize = 0x3c;
/// BL2 `FSoftSkinVertex` source record size.
pub const SOFT_SKIN_VERTEX_SIZE: usize = 0x44;

const GPU_TANGENT_X: std::ops::Range<usize> = 0..4;
const GPU_TANGENT_Z: std::ops::Range<usize> = 4..8;
const GPU_INFLUENCES: std::ops::Range<usize> = 8..16;
const GPU_POSITION: std::ops::Range<usize> = 16..28;
const GPU_UV_OFFSET: usize = 28;
const ARRAY_HEADER_BYTES: usize = size_of::<[u32; 3]>();
const SECTION_RESERVED_BYTES: usize = 3;
const LOD_INFO_PADDING_BYTES: usize = 3;
const MIN_RECONSTRUCTED_BASIS_NORM: f32 = 1e-12;

/// Two opaque FName words (index, number), not fabricated on the host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NameWords {
    /// Global name-table index (FName word0).
    pub index: u32,
    /// Disambiguating number (FName word1).
    pub number: u32,
}

/// Validated scalar fields/element bytes. Array headers are stamped natively.
#[derive(Debug)]
pub struct NativeRecords {
    /// bones x 0x50 FMeshBone records.
    pub bones: Vec<u8>,
    /// Maximum parent-chain depth encoded in the reference-bone records.
    pub skeleton_depth: u32,
    /// vertices x 0x44 FSoftSkinVertex records with up to four UV channels and BGRA8 color.
    pub soft_vertices: Vec<u8>,
    /// Concatenated per-chunk global u16 palettes, in chunk order.
    pub bone_map: Vec<u8>,
    /// Sorted active global u16 indices.
    pub active_bones: Vec<u8>,
    /// Sorted required global byte indices.
    pub required_bones: Vec<u8>,
    /// sections x 0x10 FSkelMeshSection records with actual ranges.
    pub section_records: Vec<u8>,
    /// Consecutive 0x34 FSkelMeshChunk records with unstamped nested headers.
    pub chunks: Vec<u8>,
    /// 0x3c FSkeletalMeshLODInfo. Four nested arrays stay unstamped.
    pub lod_info: Vec<u8>,
    /// LODMaterialMap elements: identity mapping, one u32 per section.
    pub lod_material_map: Vec<u8>,
    /// bEnableShadowCasting elements: one u32 (enabled) per section.
    pub shadow_elements: Vec<u8>,
    /// Native section record count and per-section shadow flag count.
    pub section_count: u32,
}

fn failure(message: &str) -> Error {
    Error(message.into())
}
fn push_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}
fn push_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}
fn push_f32(out: &mut Vec<u8>, value: f32) {
    out.extend_from_slice(&value.to_le_bytes());
}

/// Validates hierarchy/palette/weights/sections and emits native record bytes.
/// `names` must come from the SDK FName initializer at runtime.
pub fn stage(
    fixture: &SkinnedMesh,
    names: &[NameWords],
    sections: &[CheckedSection],
) -> Result<NativeRecords> {
    let skeleton: &[ReferenceBone] = &fixture.skeleton;
    if skeleton.len() != names.len() {
        return Err(failure("reference bone names must match the skeleton"));
    }
    skeleton::validate(skeleton)?;
    if names.contains(&NameWords {
        index: 0,
        number: 0,
    }) || (1..names.len()).any(|i| names[..i].contains(&names[i]))
    {
        return Err(failure(
            "reference bone names must be distinct bounded identifiers",
        ));
    }
    if fixture
        .active_bones
        .iter()
        .any(|&b| usize::from(b) >= skeleton.len())
        || fixture
            .required_bones
            .iter()
            .any(|&b| usize::from(b) >= skeleton.len())
    {
        return Err(failure("bone closure exceeds the reference skeleton"));
    }
    let skin = &fixture.skin;
    let mut vertex_cursor = 0;
    let mut index_cursor = 0;
    if skin.chunks.is_empty() || skin.chunks.len() > usize::from(u16::MAX) + 1 {
        return Err(failure("skeletal chunks require native section indices"));
    }
    for chunk in &skin.chunks {
        if chunk.base_vertex != vertex_cursor
            || chunk.first_index != index_cursor
            || chunk.vertices == 0
            || chunk.triangles == 0
            || chunk.palette.is_empty()
            || chunk.palette.len() > super::MAX_PALETTE_BONES
            || (1..chunk.palette.len()).any(|i| chunk.palette[..i].contains(&chunk.palette[i]))
            || chunk
                .palette
                .iter()
                .any(|&bone| usize::from(bone) >= skeleton.len())
        {
            return Err(failure(
                "chunk ranges and palettes must describe contiguous GPU geometry",
            ));
        }
        vertex_cursor += chunk.vertices;
        index_cursor += chunk.triangles * TRIANGLE_INDICES;
    }
    if vertex_cursor != skin.vertices || index_cursor != skin.triangles * TRIANGLE_INDICES {
        return Err(failure(
            "chunks must cover the complete vertex and index buffers",
        ));
    }
    if !(1..=4).contains(&skin.tex_coords)
        || ![size_of::<u16>(), size_of::<u32>()].contains(&skin.index_stride)
        || skin.gpu_vertices.len() != skin.vertices * skin.vertex_stride()
        || (!skin.colors.is_empty() && skin.colors.len() != skin.vertices * size_of::<[u8; 4]>())
        || skin.indices.len() != skin.triangles * TRIANGLE_INDICES * skin.index_stride
        || skin.triangles == 0
        || skin.vertices == 0
    {
        return Err(failure("skin payload counts do not match its bytes"));
    }
    if skin.vertices > u32::MAX as usize
        || skin.triangles > u32::MAX as usize
        || sections.is_empty()
        || sections.len() > crate::sections::MAX_SECTIONS
    {
        return Err(failure("skeletal record count or section count overflow"));
    }
    // CheckedSection comes from sections::validate. Re-verify the chunk/section
    // adjacency invariants this staging depends on.
    let index_count = skin.indices.len() / skin.index_stride;
    let mut cursor = 0_u32;
    for section in sections {
        if section.material_index > u32::from(u16::MAX)
            || section.first_index != cursor
            || section.triangles == 0
            || section
                .first_index
                .checked_add(section.triangles * TRIANGLE_INDICES as u32)
                .is_none_or(|end| end as usize > index_count)
        {
            return Err(failure(
                "skeletal sections must be one contiguous material partition",
            ));
        }
        cursor += section.triangles * TRIANGLE_INDICES as u32;
    }
    if cursor as usize != index_count {
        return Err(failure(
            "skeletal sections must cover the entire index list",
        ));
    }

    // FMeshBone: FName words, flags0, VJointPos rotation/translation and zero
    // editor sizes, parent, NumChildren/Depth from the hierarchy, zero BoneColor.
    // Root ParentIndex uses the UE3 importer self-parent convention (0).
    // BL2 consumers of the root parent field remain unverified.
    let children: Vec<u32> = (0..skeleton.len())
        .map(|i| {
            (1..skeleton.len())
                .filter(|&c| skeleton[c].parent == Some(i as u16))
                .count() as u32
        })
        .collect();
    let mut bones = Vec::with_capacity(skeleton.len() * BONE_SIZE);
    let mut skeleton_depth = 0;
    for (i, bone) in skeleton.iter().enumerate() {
        push_u32(&mut bones, names[i].index);
        push_u32(&mut bones, names[i].number);
        push_u32(&mut bones, 0); // Flags: not fabricated beyond zero
        push_u32(&mut bones, 0); // FMeshBone pad (VJointPos aligns at +0x10)
        for component in bone.rotation_xyzw {
            push_f32(&mut bones, component);
        }
        for axis in 0..3 {
            push_f32(&mut bones, bone.translation[axis]);
        }
        push_f32(&mut bones, 0.); // Length (editor display only)
        push_f32(&mut bones, 0.); // XSize
        push_f32(&mut bones, 0.); // YSize
        push_f32(&mut bones, 0.); // ZSize
        push_u32(&mut bones, 0); // VJointPos tail pad (0x3c..0x40)
        push_u32(&mut bones, bone.parent.unwrap_or(0) as u32); // root self-parent lead
        push_u32(&mut bones, children[i]);
        let depth = {
            let mut d = 0u32;
            let mut cursor = bone.parent;
            while let Some(parent) = cursor {
                d += 1;
                cursor = skeleton[usize::from(parent)].parent;
            }
            d
        };
        skeleton_depth = skeleton_depth.max(depth);
        push_u32(&mut bones, depth);
        push_u32(&mut bones, 0); // BoneColor
    }

    // FSoftSkinVertex from the interleaved GPU records plus a reconstructed
    // source basis: position at 0x00, packed TangentX/Y/Z at 0x0c/0x10/0x14,
    // UVs at 0x18, BGRA8 at 0x38, local influence bytes at 0x3c/0x40.
    // TangentY = sign * (Z x X) from the payload's packed/dequantized basis.
    // exact for orthonormal axis-aligned bases, approximate after quantization
    // for arbitrary imported bases.
    let mut soft_vertices = Vec::with_capacity(skin.vertices * SOFT_SKIN_VERTEX_SIZE);
    for (index, record) in skin
        .gpu_vertices
        .chunks_exact(skin.vertex_stride())
        .enumerate()
    {
        let px: [u8; PACKED_NORMAL_BYTES] =
            std::array::from_fn(|i| record[GPU_TANGENT_X.start + i]);
        let pz: [u8; PACKED_NORMAL_BYTES] =
            std::array::from_fn(|i| record[GPU_TANGENT_Z.start + i]);
        let sign = if pz[PACKED_NORMAL_BYTES - 1] >= PACKED_NORMAL_MIDPOINT {
            1.
        } else {
            -1.
        };
        let x = unpack_normal(px);
        let z = unpack_normal(pz);
        let y = [
            sign * (z[1] * x[2] - z[2] * x[1]),
            sign * (z[2] * x[0] - z[0] * x[2]),
            sign * (z[0] * x[1] - z[1] * x[0]),
        ];
        // Byte quantization makes dequantized X/Z slightly non-unit and slightly
        // non-orthogonal, so |Z x X| can reach ~1.01 and would trip pack_normal's
        // strict component bound. TangentY is a direction: renormalize the
        // reconstruction instead of rejecting real rotated geometry. Only a
        // degenerate (zero/nonfinite) basis is an error.
        let y_norm = y.iter().map(|v| v * v).sum::<f32>();
        if !y_norm.is_finite() || y_norm < MIN_RECONSTRUCTED_BASIS_NORM {
            return Err(Error("degenerate reconstructed tangent basis".into()));
        }
        let y_inv = 1. / y_norm.sqrt();
        let y = [y[0] * y_inv, y[1] * y_inv, y[2] * y_inv];
        let packed_y = crate::encoding::pack_normal(y)?;
        soft_vertices.extend_from_slice(&record[GPU_POSITION]);
        soft_vertices.extend_from_slice(&record[GPU_TANGENT_X]);
        soft_vertices.extend_from_slice(&packed_y);
        soft_vertices.extend_from_slice(&record[GPU_TANGENT_Z]);
        soft_vertices.extend_from_slice(&record[GPU_UV_OFFSET..]);
        for _ in skin.tex_coords..4 {
            soft_vertices.extend_from_slice(&[0; size_of::<[f32; 2]>()]);
        }
        if skin.colors.is_empty() {
            soft_vertices.extend_from_slice(&[255; 4]);
        } else {
            soft_vertices.extend_from_slice(&skin.colors[index * 4..(index + 1) * 4]);
        }
        soft_vertices.extend_from_slice(&record[GPU_INFLUENCES]);
    }

    let bone_map = skin
        .chunks
        .iter()
        .flat_map(|chunk| chunk.palette.iter().flat_map(|bone| bone.to_le_bytes()))
        .collect();
    let active_bones = fixture
        .active_bones
        .iter()
        .flat_map(|g| g.to_le_bytes())
        .collect();
    let required_bones = fixture.required_bones.clone();

    // Material boundaries and palette boundaries independently split draw sections.
    // Their intersection preserves triangle order and existing Gestalt index ranges.
    let mut section_records = Vec::new();
    let mut chunk_index = 0;
    for section in sections {
        let end = section.first_index as usize + section.triangles as usize * TRIANGLE_INDICES;
        let mut first = section.first_index as usize;
        while first < end {
            let chunk = &skin.chunks[chunk_index];
            let chunk_end = chunk.first_index + chunk.triangles * TRIANGLE_INDICES;
            let next = end.min(chunk_end);
            push_u16(&mut section_records, section.material_index as u16);
            push_u16(&mut section_records, chunk_index as u16);
            push_u32(&mut section_records, first as u32);
            push_u32(
                &mut section_records,
                ((next - first) / TRIANGLE_INDICES) as u32,
            );
            section_records.push(0);
            section_records.extend_from_slice(&[0; SECTION_RESERVED_BYTES]);
            first = next;
            if next == chunk_end {
                chunk_index += 1;
            }
        }
    }
    let section_count = section_records.len() / SECTION_SIZE;
    if section_count > i32::MAX as usize {
        return Err(failure("chunked section count exceeds native limits"));
    }
    let mut chunks = Vec::with_capacity(skin.chunks.len() * CHUNK_SIZE);
    for chunk in &skin.chunks {
        push_u32(&mut chunks, chunk.base_vertex as u32);
        chunks.extend_from_slice(&[0; ARRAY_HEADER_BYTES * 3]);
        push_u32(&mut chunks, 0);
        push_u32(&mut chunks, chunk.vertices as u32);
        push_u32(&mut chunks, chunk.max_bone_influences);
    }

    // FSkeletalMeshLODInfo: one LOD, per-section maps/sort arrays unstamped.
    // DisplayFactor 1.0: neutral for a single-LOD proxy (UE3 consumer lead:
    // larger selects this LOD sooner). LODHysteresis 0 (no multi-LOD bias).
    let mut lod_info = Vec::with_capacity(LOD_INFO_SIZE);
    push_f32(&mut lod_info, 1.);
    push_f32(&mut lod_info, 0.);
    lod_info.extend_from_slice(&[0; ARRAY_HEADER_BYTES]); // LODMaterialMap
    lod_info.extend_from_slice(&[0; ARRAY_HEADER_BYTES]); // bEnableShadowCasting
    lod_info.extend_from_slice(&[0; ARRAY_HEADER_BYTES]); // OLD_TriangleSorting
    lod_info.extend_from_slice(&[0; ARRAY_HEADER_BYTES]); // TriangleSortSettings
    lod_info.push(0); // bDisableCompression/bHasBeenSimplified bits
    lod_info.extend_from_slice(&[0; LOD_INFO_PADDING_BYTES]); // record pad to 0x3c

    let material_count = sections
        .iter()
        .map(|s| s.material_index as usize + 1)
        .max()
        .unwrap_or(0);
    let mut lod_material_map = Vec::with_capacity(material_count * size_of::<u32>());
    for i in 0..material_count {
        push_u32(&mut lod_material_map, i as u32);
    }
    let mut shadow_elements = Vec::with_capacity(section_count * size_of::<u32>());
    for _ in 0..section_count {
        push_u32(&mut shadow_elements, 1);
    }

    Ok(NativeRecords {
        bones,
        skeleton_depth,
        soft_vertices,
        bone_map,
        active_bones,
        required_bones,
        section_records,
        chunks,
        lod_info,
        lod_material_map,
        shadow_elements,
        section_count: section_count as u32,
    })
}
