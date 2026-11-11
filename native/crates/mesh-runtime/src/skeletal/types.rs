//! Engine-space skin inputs and validated payload records.

use crate::encoding::Bounds;

/// One vertex's raw skinned inputs: up to four GLOBAL skeleton joints.
#[derive(Debug, Clone, Copy)]
pub struct SkinWeights {
    /// Global skeleton indices (zero-weight slots are canonicalized later).
    pub joints: [u16; 4],
    /// Finite nonnegative weights. Nonzero sum is enforced per vertex.
    pub weights: [f32; 4],
}

/// Reference-pose bone, not a packed FMeshBone/FBoneAtom or runtime FName.
#[derive(Debug, Clone)]
pub struct ReferenceBone {
    /// Bone name. New native skeletons intern it through the SDK FName path.
    pub name: String,
    /// Semantic parent. Bones are parent-before-child. Exactly one root: index 0.
    pub parent: Option<u16>,
    /// Parent-relative translation in engine units.
    pub translation: [f32; 3],
    /// Parent-relative XYZW unit quaternion.
    pub rotation_xyzw: [f32; 4],
}

/// One contiguous GPU vertex range with its own shader bone palette.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SkinChunk {
    /// First vertex in the shared LOD vertex buffer.
    pub base_vertex: usize,
    /// Number of soft vertices owned by this chunk.
    pub vertices: usize,
    /// First triangle-list index belonging to this chunk.
    pub first_index: usize,
    /// Number of consecutive triangles using this palette.
    pub triangles: usize,
    /// Local influence byte to global reference-bone mapping.
    pub palette: Vec<u16>,
    /// Maximum nonzero influence count among this chunk's vertices.
    pub max_bone_influences: u32,
}

/// Rust-owned GPU skin staging before native allocation and attachment.
#[derive(Debug, Clone)]
pub struct SkinPayload {
    /// Interleaved full-XYZ records with one through four full-precision UV pairs.
    pub gpu_vertices: Vec<u8>,
    /// Separate native BGRA8 color stream, or empty when colors are absent.
    pub colors: Vec<u8>,
    /// Native texture-coordinate count, between one and four.
    pub tex_coords: usize,
    /// Little-endian triangle-list u16 or u32 indices.
    pub indices: Vec<u8>,
    /// Native multi-size index width, either two or four bytes.
    pub index_stride: usize,
    /// Unique weighted global bones across all chunks, not a GPU-local lookup table.
    pub palette: Vec<u16>,
    /// Contiguous vertex and triangle partitions with independent local palettes.
    pub chunks: Vec<SkinChunk>,
    /// Validated number of skin records.
    pub vertices: usize,
    /// Validated number of triangles.
    pub triangles: usize,
    /// Largest nonzero influence count after quantization.
    pub max_bone_influences: u32,
    /// Reference-pose geometry bounds only, NOT arbitrary animated bounds.
    pub bounds: Bounds,
}

impl SkinPayload {
    /// Returns the packed tangent, influence, position and full-UV record stride.
    pub fn vertex_stride(&self) -> usize {
        28 + self.tex_coords * size_of::<[f32; 2]>()
    }
}

/// A validated chunked skinned mesh plus its reference skeleton.
#[derive(Debug, Clone)]
pub struct SkinnedMesh {
    /// Encoded GPU records, indices, palette and reference-pose bounds.
    pub skin: SkinPayload,
    /// Parent-before-child reference bones. Index 0 is the only root.
    pub skeleton: Vec<ReferenceBone>,
    /// Sorted global u16 bones referenced with nonzero weight.
    pub active_bones: Vec<u16>,
    /// Every reference bone in parent-before-child order, including unweighted helpers.
    pub required_bones: Vec<u8>,
}
