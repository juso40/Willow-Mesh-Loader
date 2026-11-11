//! Shared native static LOD staging and publication for population and replacement.

use crate::engine::{
    abi::{ARRAY_HEADER_BYTES, WORD_BYTES, vertex_impl},
    colors,
    construct::{self, Region, retain_allocation, retain_bytes},
    native,
};
use crate::static_mesh::{
    bindings::Geometry,
    layout::{lod as lod_layout, mesh as mesh_layout, position, tangent},
};
use crate::{
    Result,
    encoding::{POSITION_STRIDE, Payload, TANGENT_UV_STRIDE, TRIANGLE_INDICES},
};

#[derive(Clone, Copy)]
enum VertexBufferKind {
    Position,
    TangentUv,
}

fn fill_buffer(
    api: &Geometry,
    buffer: Region,
    payload: &[u8],
    vertices: u32,
    kind: VertexBufferKind,
) -> Result<()> {
    if matches!(kind, VertexBufferKind::TangentUv) {
        buffer.u32(tangent::TEX_COORDS, 1)?;
        buffer.u32(tangent::FULL_PRECISION_UVS, 1)?;
    }
    let (stride, data_field, stride_field, count_field) = match kind {
        VertexBufferKind::TangentUv => (
            TANGENT_UV_STRIDE,
            tangent::DATA,
            tangent::STRIDE,
            tangent::VERTICES,
        ),
        VertexBufferKind::Position => (
            POSITION_STRIDE,
            position::DATA,
            position::STRIDE,
            position::VERTICES,
        ),
    };
    // SAFETY: the checked buffer region has the selected native vertex-buffer layout.
    unsafe {
        match kind {
            VertexBufferKind::TangentUv => {
                (api.tangent_allocate)(buffer.address as *mut native::TangentBuffer)
            }
            VertexBufferKind::Position => {
                (api.position_allocate)(buffer.address as *mut native::PositionBuffer)
            }
        }
    }
    let implementation = construct::vertex_storage(buffer.address + tangent::IMPLEMENTATION)?;
    let storage = retain_bytes(payload)?;
    // Complete GMalloc storage is published into the constructor-owned empty array.
    implementation.header(vertex_impl::ARRAY, storage.address, vertices)?;
    buffer.u32(data_field, storage.address as u32)?;
    buffer.u32(stride_field, stride as u32)?;
    buffer.u32(count_field, vertices)
}

pub(crate) struct StagedGeometry {
    indirect: Region,
    lod_info: Region,
}

pub(crate) fn stage(
    api: &Geometry,
    payload: &Payload,
    section_bytes: &[u8],
    section_count: usize,
) -> Result<StagedGeometry> {
    let lod = retain_allocation(lod_layout::SIZE)?;
    // SAFETY: the retained allocation has the reviewed StaticLod size and alignment.
    unsafe { (api.lod_constructor)(lod.address as *mut native::StaticLod) };
    fill_buffer(
        api,
        Region {
            address: lod.address,
            length: tangent::SIZE,
        },
        &payload.tangent_uv,
        payload.vertices as u32,
        VertexBufferKind::TangentUv,
    )?;
    fill_buffer(
        api,
        Region {
            address: lod.address + lod_layout::POSITION,
            length: position::SIZE,
        },
        &payload.positions,
        payload.vertices as u32,
        VertexBufferKind::Position,
    )?;
    colors::fill(
        api.color_allocate,
        Region {
            address: lod.address + lod_layout::COLORS,
            length: colors::SIZE,
        },
        &payload.colors,
        payload.vertices,
    )?;
    let indices = retain_bytes(&payload.indices)?;
    lod.header(
        lod_layout::INDICES,
        indices.address,
        (payload.triangles * TRIANGLE_INDICES) as u32,
    )?;
    lod.u32(lod_layout::INDEX_CPU_ACCESS, 1)?;
    lod.u32(lod_layout::INDEX_RESOURCE, 0)?;
    lod.u32(lod_layout::INDEX_RESOURCE_FLAGS, 0)?;
    lod.u32(lod_layout::VERTICES, payload.vertices as u32)?;
    let section = retain_bytes(section_bytes)?;
    lod.header(lod_layout::SECTIONS, section.address, section_count as u32)?;
    let lod_info = retain_allocation(ARRAY_HEADER_BYTES)?;
    let indirect = retain_allocation(WORD_BYTES)?;
    indirect.u32(0, lod.address as u32)?;
    Ok(StagedGeometry { indirect, lod_info })
}

pub(crate) fn install(mesh: Region, staged: &StagedGeometry, payload: &Payload) -> Result<()> {
    for index in 0..payload.bounds.origin.len() {
        mesh.f32(
            mesh_layout::BOUNDS_ORIGIN + index * WORD_BYTES,
            payload.bounds.origin[index],
        )?;
        mesh.f32(
            mesh_layout::BOUNDS_EXTENT + index * WORD_BYTES,
            payload.bounds.box_extent[index],
        )?;
    }
    mesh.f32(mesh_layout::BOUNDS_RADIUS, payload.bounds.sphere_radius)?;
    // All pointees are complete before either mesh-owned header becomes counted.
    mesh.header(mesh_layout::LOD_INFO, staged.lod_info.address, 1)?;
    mesh.header(mesh_layout::LOD_MODELS, staged.indirect.address, 1)
}
