//! Skeletal operation bindings resolved before native storage changes.
//!
//! Signature and relative-call checks produce the addresses used throughout one
//! synchronous operation. The caller excludes concurrent native code patching.

use crate::{
    Result,
    engine::{discovery::Table, lifecycle::Lifecycle, native::*, signatures::*},
};

/// Shared LOD construction dependencies for creation and replacement.
pub(crate) struct Geometry {
    pub(crate) lod_constructor: Method<SkeletalLod>,
    pub(crate) allocate_data: Method<SkinBuffer>,
    pub(crate) color_allocate: Method<ColorBuffer>,
    pub(crate) index_vtable: Table,
    pub(crate) index_array_vtable: Table,
}

impl Geometry {
    fn resolve(base: usize, index_stride: usize) -> Result<Self> {
        let (index_vtable, index_array_vtable) = match index_stride {
            2 => (&SKIN_INDEX_VTABLE, &STATIC_INDEX_ARRAY_VTABLE),
            4 => (&SKIN_INDEX32_VTABLE, &SKIN_INDEX32_ARRAY_VTABLE),
            _ => return Err(crate::Error("unsupported skeletal index width".into())),
        };
        Ok(Self {
            lod_constructor: SKELETAL_LOD_CONSTRUCTOR.resolve(base)?,
            allocate_data: SKIN_ALLOCATE_DATA.resolve(base)?,
            color_allocate: SKIN_COLOR_ALLOCATE_DATA.resolve(base)?,
            index_vtable: index_vtable.resolve(base)?,
            index_array_vtable: index_array_vtable.resolve(base)?,
        })
    }
}

/// Geometry initialization without skeleton-cache builders.
pub(crate) struct Storage {
    pub(crate) geometry: Geometry,
    pub(crate) init_resources: Method<SkeletalMesh>,
}

impl Storage {
    fn resolve(base: usize, index_stride: usize) -> Result<Self> {
        Ok(Self {
            geometry: Geometry::resolve(base, index_stride)?,
            init_resources: SKELETAL_INIT_RESOURCES.resolve(base)?,
        })
    }
}

/// Initial construction dependencies including reference-skeleton cache builders.
pub(crate) struct Create {
    pub(crate) storage: Storage,
    pub(crate) init_name_index_map: Method<SkeletalMesh>,
    pub(crate) calculate_inv_ref_matrices: Method<SkeletalMesh>,
    pub(crate) build_ref_bone_atom_cache: Method<SkeletalMesh>,
}

impl Create {
    pub(crate) fn resolve(base: usize, index_stride: usize) -> Result<Self> {
        Ok(Self {
            storage: Storage::resolve(base, index_stride)?,
            init_name_index_map: INIT_NAME_INDEX_MAP.resolve(base)?,
            calculate_inv_ref_matrices: CALCULATE_INV_REF_MATRICES.resolve(base)?,
            build_ref_bone_atom_cache: BUILD_REF_BONE_ATOM_CACHE.resolve(base)?,
        })
    }
}

/// Replacement dependencies that leave the existing reference skeleton intact.
pub(crate) struct Reload {
    pub(crate) storage: Storage,
    pub(crate) lifecycle: Lifecycle<SkeletalMesh>,
    pub(crate) mesh_vtable: Table,
    pub(crate) component_attach: usize,
    pub(crate) component_detach: usize,
    pub(crate) component_valid: usize,
}

impl Reload {
    pub(crate) fn resolve(base: usize, index_stride: usize) -> Result<Self> {
        Ok(Self {
            storage: Storage::resolve(base, index_stride)?,
            lifecycle: Lifecycle::resolve(base, &SKELETAL_RELEASE_RESOURCES)?,
            mesh_vtable: SKELETAL_MESH_VTABLE.resolve(base)?,
            component_attach: SKELETAL_COMPONENT_ATTACH.address(base)?,
            component_detach: SKELETAL_COMPONENT_DETACH.address(base)?,
            component_valid: SKELETAL_COMPONENT_VALID.address(base)?,
        })
    }
}
