//! Reviewed static mesh storage and component lifecycle fields.

pub(crate) mod mesh {
    pub(crate) const SIZE: usize = 0x130;
    pub(crate) const LOD_MODELS: usize = 0x3c;
    pub(crate) const LOD_INFO: usize = 0x48;
    pub(crate) const LOD_DISTANCE_RATIO: usize = 0x54;
    pub(crate) const LOD_MAX_RANGE: usize = 0x58;
    pub(crate) const LIGHT_MAP_RESOLUTION: usize = 0x5c;
    pub(crate) const LIGHT_MAP_COORDINATE_INDEX: usize = 0x60;
    pub(crate) const COLLISION_NODES: usize = 0x70;
    pub(crate) const COLLISION_TRIANGLES: usize = 0x7c;
    pub(crate) const BODY_SETUP: usize = 0xb0;
    pub(crate) const BOUNDS_ORIGIN: usize = 0xb4;
    pub(crate) const BOUNDS_EXTENT: usize = 0xc0;
    pub(crate) const BOUNDS_RADIUS: usize = 0xcc;
    pub(crate) const FULL_PRECISION_UVS: usize = 0xf8;
    pub(crate) const USED_FOR_INSTANCING: usize = 0xfc;
    pub(crate) const CAN_BECOME_DYNAMIC: usize = 0x104;
    pub(crate) const STREAMING_DISTANCE_MULTIPLIER: usize = 0x108;
    pub(crate) const RELEASE_FENCE: usize = 0x11c;
    pub(crate) const DEFAULT_LOD_MAX_RANGE: f32 = 2000.;
}

pub(crate) mod lod {
    pub(crate) const SIZE: usize = 0x364;
    pub(crate) const POSITION: usize = 0x30;
    pub(crate) const COLORS: usize = 0x58;
    pub(crate) const VERTICES: usize = 0x80;
    pub(crate) const INDICES: usize = 0xa0;
    pub(crate) const INDEX_CPU_ACCESS: usize = 0xac;
    pub(crate) const INDEX_RESOURCE: usize = 0xb0;
    pub(crate) const INDEX_RESOURCE_FLAGS: usize = 0xb4;
    pub(crate) const SECTIONS: usize = 0xdc;
}

pub(crate) mod tangent {
    pub(crate) const SIZE: usize = 0x30;
    pub(crate) const IMPLEMENTATION: usize = 0x18;
    pub(crate) const TEX_COORDS: usize = 0x1c;
    pub(crate) const DATA: usize = 0x20;
    pub(crate) const STRIDE: usize = 0x24;
    pub(crate) const VERTICES: usize = 0x28;
    pub(crate) const FULL_PRECISION_UVS: usize = 0x2c;
}

pub(crate) mod position {
    pub(crate) const SIZE: usize = 0x28;
    pub(crate) const DATA: usize = 0x1c;
    pub(crate) const STRIDE: usize = 0x20;
    pub(crate) const VERTICES: usize = 0x24;
}

pub(crate) mod component {
    pub(crate) const MESH: usize = 0x21c;
    pub(crate) const LOD_DATA_COUNT: usize = 0x23c;
    pub(crate) const ATTACH_DISPATCH: usize = 0x150;
    pub(crate) const DETACH_DISPATCH: usize = 0x158;
    pub(crate) const VALIDITY_DISPATCH: usize = 0x16c;
}
