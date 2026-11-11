//! Reviewed skeletal mesh storage and component lifecycle fields.

pub(crate) mod mesh {
    pub(crate) const SIZE: usize = 0x3b4;
    pub(crate) const BOUNDS_ORIGIN: usize = 0x3c;
    pub(crate) const BOUNDS_EXTENT: usize = 0x48;
    pub(crate) const BOUNDS_RADIUS: usize = 0x54;
    pub(crate) const MATERIALS: usize = 0x58;
    pub(crate) const REFERENCE_SKELETON: usize = 0x88;
    pub(crate) const SKELETON_DEPTH: usize = 0x94;
    pub(crate) const LOD_MODELS: usize = 0xd4;
    pub(crate) const RENDER_BOUNDS_ORIGIN: usize = 0xec;
    pub(crate) const RENDER_BOUNDS_EXTENT: usize = 0xf8;
    pub(crate) const RENDER_BOUNDS_RADIUS: usize = 0x104;
    pub(crate) const LOD_INFO: usize = 0x13c;
    pub(crate) const SKINNING_FLAGS: usize = 0x16c;
    pub(crate) const COLOR_FLAGS: usize = 0x374;
    pub(crate) const RELEASE_FENCE: usize = 0x394;
    pub(crate) const REFERENCE_ATOMS: usize = 0x3a0;
    pub(crate) const HAS_VERTEX_COLORS: u32 = 1 << 1;
    pub(crate) const FORCE_CPU_SKINNING: u32 = 1 << 3;
    pub(crate) const FULL_PRECISION_UVS: u32 = 1 << 4;
}

pub(crate) mod lod {
    pub(crate) const SIZE: usize = 0x13c;
    pub(crate) const SECTIONS: usize = 0;
    pub(crate) const CHUNKS: usize = 0x0c;
    pub(crate) const ACTIVE_BONES: usize = 0x18;
    pub(crate) const REQUIRED_BONES: usize = 0x24;
    pub(crate) const INDEX_CPU_ACCESS: usize = 0x30;
    pub(crate) const INDEX_TYPE_SIZE: usize = 0x34;
    pub(crate) const INDEX_RESOURCE: usize = 0x38;
    pub(crate) const RENDER_BYTES: usize = 0x3c;
    pub(crate) const VERTICES: usize = 0x40;
    pub(crate) const TEX_COORDS: usize = 0x44;
    pub(crate) const GPU_RESOURCE: usize = 0x48;
    pub(crate) const FULL_PRECISION_UVS: usize = 0x64;
    pub(crate) const PACKED_POSITION: usize = 0x6c;
    pub(crate) const PROCESSED_PACKED_POSITION: usize = 0x70;
    pub(crate) const GPU_IMPLEMENTATION: usize = 0x74;
    pub(crate) const GPU_DATA: usize = 0x78;
    pub(crate) const GPU_STRIDE: usize = 0x7c;
    pub(crate) const GPU_VERTICES: usize = 0x80;
    pub(crate) const GPU_TEX_COORDS: usize = 0x84;
    pub(crate) const COLORS: usize = 0xa0;
}

pub(crate) mod index_resource {
    pub(crate) const SIZE: usize = 0x34;
    pub(crate) const ARRAY_VTABLE: usize = 0x20;
    pub(crate) const INDICES: usize = 0x24;
    pub(crate) const CPU_ACCESS: usize = 0x30;
}

pub(crate) mod chunk {
    pub(crate) const BASE_VERTEX: usize = 0;
    pub(crate) const RIGID_COUNT: usize = 0x28;
    pub(crate) const SOFT_COUNT: usize = 0x2c;
    pub(crate) const SOFT_VERTICES: usize = 0x10;
    pub(crate) const BONE_MAP: usize = 0x1c;
}

pub(crate) mod lod_info {
    pub(crate) const MATERIAL_MAP: usize = 0x08;
    pub(crate) const SHADOW_ELEMENTS: usize = 0x14;
}

pub(crate) mod component {
    pub(crate) const MESH: usize = 0x220;
    pub(crate) const RENDER_OBJECT: usize = 0x2d8;
    pub(crate) const LOD_DATA: usize = 0x458;
    pub(crate) const LOD_DATA_COUNT: usize = 0x45c;
    pub(crate) const ATTACH_DISPATCH: usize = 0x150;
    pub(crate) const DETACH_DISPATCH: usize = 0x158;
    pub(crate) const VALIDITY_DISPATCH: usize = 0x16c;
    pub(crate) const LOD_INFLUENCES: usize = 4;
    pub(crate) const LOD_FLAGS: usize = 0x0c;
    pub(crate) const LOD_SKIN_WEIGHTS: usize = 0x10;
    pub(crate) const LOD_INFLUENCE_INDEX: usize = 0x14;
    pub(crate) const OVERRIDE_INFLUENCES: u32 = 3;
}
