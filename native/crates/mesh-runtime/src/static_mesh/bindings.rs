//! Static operation bindings resolved before native storage changes.
//!
//! Signature and relative-call checks produce the addresses used throughout one
//! synchronous operation. The caller excludes concurrent native code patching.

use crate::{
    Result,
    engine::{discovery::Table, lifecycle::Lifecycle, native::*, signatures::*},
};

/// Shared LOD construction dependencies for creation and replacement.
pub(crate) struct Geometry {
    pub(crate) lod_constructor: Method<StaticLod>,
    pub(crate) tangent_allocate: Method<TangentBuffer>,
    pub(crate) position_allocate: Method<PositionBuffer>,
    pub(crate) color_allocate: Method<ColorBuffer>,
}

impl Geometry {
    fn resolve(base: usize) -> Result<Self> {
        Ok(Self {
            lod_constructor: STATIC_LOD_CONSTRUCTOR.resolve(base)?,
            tangent_allocate: TANGENT_ALLOCATE_DATA.resolve(base)?,
            position_allocate: POSITION_ALLOCATE_DATA.resolve(base)?,
            color_allocate: STATIC_COLOR_ALLOCATE_DATA.resolve(base)?,
        })
    }
}

/// Complete storage initialization dependencies.
pub(crate) struct Storage {
    pub(crate) geometry: Geometry,
    pub(crate) init_resources: Method<StaticMesh>,
}

impl Storage {
    pub(crate) fn resolve(base: usize) -> Result<Self> {
        Ok(Self {
            geometry: Geometry::resolve(base)?,
            init_resources: STATIC_INIT_RESOURCES.resolve(base)?,
        })
    }
}

/// Replacement dependencies including component dispatch and resource release.
pub(crate) struct Reload {
    pub(crate) storage: Storage,
    pub(crate) lifecycle: Lifecycle<StaticMesh>,
    pub(crate) mesh_vtable: Table,
    pub(crate) component_vtable: Table,
    pub(crate) component_attach: usize,
    pub(crate) component_detach: usize,
    pub(crate) component_valid: usize,
}

impl Reload {
    pub(crate) fn resolve(base: usize) -> Result<Self> {
        Ok(Self {
            storage: Storage::resolve(base)?,
            lifecycle: Lifecycle::resolve(base, &STATIC_RELEASE_RESOURCES)?,
            mesh_vtable: STATIC_MESH_VTABLE.resolve(base)?,
            component_vtable: STATIC_COMPONENT_VTABLE.resolve(base)?,
            component_attach: STATIC_COMPONENT_ATTACH.address(base)?,
            component_detach: STATIC_COMPONENT_DETACH.address(base)?,
            component_valid: STATIC_COMPONENT_VALID.address(base)?,
        })
    }
}
