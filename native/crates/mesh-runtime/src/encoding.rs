//! Offline BL2 compact/full-UV1 payload encoding. No engine objects, pointers,
//! allocation callbacks, publication, coordinate conversion, or file parsing.
//! Rust Vec backing must be copied into correctly constructed engine-owned storage.

use crate::{Error, Result};

/// Number of index elements in one triangle-list primitive.
pub const TRIANGLE_INDICES: usize = 3;
/// Byte stride of an XYZ position record.
pub const POSITION_STRIDE: usize = size_of::<[f32; 3]>();
/// Byte size of one native packed normal.
pub const PACKED_NORMAL_BYTES: usize = size_of::<[u8; 4]>();
/// Byte size of the packed tangent X/Z prefix.
pub const PACKED_TANGENTS_BYTES: usize = PACKED_NORMAL_BYTES * 2;
/// Byte stride of packed tangent X/Z and one full-precision UV pair.
pub const TANGENT_UV_STRIDE: usize = PACKED_TANGENTS_BYTES + size_of::<[f32; 2]>();
/// Byte stride of the compact native index format.
pub const INDEX_STRIDE: usize = size_of::<u16>();
/// Number of vertices addressable by the compact index format.
pub const MAX_COMPACT_VERTICES: usize = u16::MAX as usize + 1;
/// Unit-vector and orthogonality tolerance for native tangent encoding.
pub const BASIS_TOLERANCE: f32 = 0.001;
/// Exact reciprocal packing multiplier used by native packed-normal consumers.
pub const NORMAL_UNPACK_SCALE: f32 = f32::from_bits(0x3c008081);
/// Midpoint byte used for an otherwise unused packed-normal W lane.
pub const PACKED_NORMAL_MIDPOINT: u8 = 128;
const NORMAL_PACK_SCALE: f32 = u8::MAX as f32 / 2.;
const MIN_PACKED_BASIS_DETERMINANT: f32 = 0.5;

/// One vertex in already-converted engine coordinates with an orthonormal basis.
#[derive(Debug, Clone, Copy)]
pub struct Vertex {
    /// Finite local-space position. Units are supplied by the caller.
    pub position: [f32; 3],
    /// Unit tangent direction.
    pub tangent_x: [f32; 3],
    /// Unit bitangent. Required to determine handedness after packing.
    pub tangent_y: [f32; 3],
    /// Unit normal direction.
    pub tangent_z: [f32; 3],
    /// One finite full-precision UV pair (not restricted to [0, 1]).
    pub uv: [f32; 2],
    /// Linear RGBA8 color, or absent. Missing colors become opaque white in mixed streams.
    pub color: Option<[u8; 4]>,
}

/// Finite local bounds, deliberately conservative after f32 conversion.
#[derive(Debug, Clone, Copy)]
pub struct Bounds {
    /// Local center (origin of the FBoxSphereBounds, not world placement).
    pub origin: [f32; 3],
    /// Half extent enclosing all input positions.
    pub box_extent: [f32; 3],
    /// Radius enclosing all input positions relative to the rounded center.
    pub sphere_radius: f32,
}

/// Host-owned bytes only. Not a native LOD or a published asset.
#[derive(Debug)]
pub struct Payload {
    /// Little-endian XYZ floats, 12 bytes per vertex.
    pub positions: Vec<u8>,
    /// Packed X and Z (including sign in Z.W), then UV floats: 16 bytes per vertex.
    pub tangent_uv: Vec<u8>,
    /// Native BGRA8 colors, four bytes per vertex, or empty when no colors are supplied.
    pub colors: Vec<u8>,
    /// Little-endian u16 or u32 indices. Values are never truncated.
    pub indices: Vec<u8>,
    /// Index-element width in bytes, either two or four.
    pub index_stride: usize,
    /// Vertex count after validation.
    pub vertices: usize,
    /// Triangle count after validation.
    pub triangles: usize,
    /// Position-derived bounds. No collision body is created.
    pub bounds: Bounds,
}

fn failure(message: &str) -> Error {
    Error(message.into())
}
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Encodes scalar FPackedNormal components as observed in both builds. Zero becomes 127,
/// not 128. W is 128 until the compact normal's handedness replaces it.
/// Only finite, bounded inputs are accepted (avoids engine overflow/NaN quirks).
pub fn pack_normal(vector: [f32; 3]) -> Result<[u8; 4]> {
    if vector
        .iter()
        .any(|v| !v.is_finite() || v.abs() > 1. + BASIS_TOLERANCE)
    {
        return Err(failure("packed normal requires finite bounded components"));
    }
    let mut packed = [0, 0, 0, PACKED_NORMAL_MIDPOINT];
    for i in 0..3 {
        // Separate f32 add/multiply, CVTTSS2SI truncation, then clamp.
        packed[i] = (((vector[i] + 1.0) * NORMAL_PACK_SCALE) as i32).clamp(0, u8::MAX as i32) as u8;
    }
    Ok(packed)
}

pub(crate) fn unpack_normal(packed: [u8; 4]) -> [f32; 3] {
    // GVectorPackingConstants lane 2, from the BL2 initializer's source bytes.
    let scale = NORMAL_UNPACK_SCALE;
    [
        packed[0] as f32 * scale - 1.0,
        packed[1] as f32 * scale - 1.0,
        packed[2] as f32 * scale - 1.0,
    ]
}

fn basis(vertex: &Vertex) -> Result<([u8; 4], [u8; 4])> {
    let directions = [vertex.tangent_x, vertex.tangent_y, vertex.tangent_z];
    for direction in directions {
        if direction.iter().any(|v| !v.is_finite())
            || (dot(direction, direction) - 1.0).abs() > BASIS_TOLERANCE
        {
            return Err(failure("tangent basis must contain finite unit directions"));
        }
    }
    for (a, b) in [(0, 1), (0, 2), (1, 2)] {
        if dot(directions[a], directions[b]).abs() > BASIS_TOLERANCE {
            return Err(failure("tangent basis must be orthogonal"));
        }
    }
    let x = pack_normal(directions[0])?;
    let y = pack_normal(directions[1])?;
    let mut z = pack_normal(directions[2])?;
    // Engine consumers determine sign from the DEQUANTIZED packed basis,
    // not the unquantized glTF basis or an arbitrary tangent-W convention.
    let px = unpack_normal(x);
    let py = unpack_normal(y);
    let pz = unpack_normal(z);
    let determinant = ((pz[2] * py[1] - py[2] * pz[1]) * px[0]
        - (pz[2] * px[1] - pz[1] * px[2]) * py[0])
        + (py[2] * px[1] - py[1] * px[2]) * pz[0];
    if !determinant.is_finite() || determinant.abs() < MIN_PACKED_BASIS_DETERMINANT {
        return Err(failure("unstable packed tangent basis"));
    }
    z[3] = if determinant >= 0.0 { u8::MAX } else { 0 };
    Ok((x, z))
}

fn enclosing_f32(value: f64) -> Result<f32> {
    if !value.is_finite() || value < 0.0 || value > f32::MAX as f64 {
        return Err(failure("bounds exceed finite f32 range"));
    }
    let mut rounded = value as f32;
    if (rounded as f64) < value {
        rounded = f32::from_bits(rounded.to_bits() + 1);
    }
    if !rounded.is_finite() {
        return Err(failure("bounds exceed finite f32 range"));
    }
    Ok(rounded)
}

/// Computes finite enclosing bounds for a nonempty collection of finite positions.
pub fn bounds(vertices: &[Vertex]) -> Result<Bounds> {
    bounds_from_positions(vertices.iter().map(|vertex| vertex.position))
}

/// Computes enclosing bounds from repeatable positions without copying vertex records.
pub fn bounds_from_positions(positions: impl Iterator<Item = [f32; 3]> + Clone) -> Result<Bounds> {
    if positions.clone().next().is_none()
        || positions
            .clone()
            .any(|position| position.iter().any(|p| !p.is_finite()))
    {
        return Err(failure("bounds require nonempty finite positions"));
    }
    let mut min = [f64::INFINITY; 3];
    let mut max = [f64::NEG_INFINITY; 3];
    for position in positions.clone() {
        for i in 0..3 {
            min[i] = min[i].min(position[i] as f64);
            max[i] = max[i].max(position[i] as f64);
        }
    }
    let mut origin = [0f32; 3];
    let mut extent = [0f32; 3];
    for i in 0..3 {
        origin[i] = ((min[i] + max[i]) * 0.5) as f32;
        let center = origin[i] as f64;
        extent[i] = enclosing_f32((max[i] - center).abs().max((min[i] - center).abs()))?;
    }
    let radius_squared = positions
        .map(|position| {
            position
                .iter()
                .zip(origin)
                .map(|(&p, c)| (p as f64 - c as f64).powi(2))
                .sum::<f64>()
        })
        .fold(0f64, f64::max);
    Ok(Bounds {
        origin,
        box_extent: extent,
        sphere_radius: enclosing_f32(radius_squared.sqrt())?,
    })
}

/// Reserves host payload bytes without aborting on allocation or capacity failure.
pub(crate) fn buffer(size: usize) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(size)
        .map_err(|_| failure("host payload allocation failed"))?;
    Ok(bytes)
}

/// Encodes one-UV triangle-list bytes without SDK/game access. Callers must have
/// applied coordinate/unit/winding conversion. Does not generate normals, weld,
/// split primitives, choose materials, construct native headers, or invoke RHI.
/// Distinct-index collinear triangles are not detected by this low-level encoder.
pub fn encode(vertices: &[Vertex], indices: &[u32]) -> Result<Payload> {
    encode_with_index_width(vertices, indices, INDEX_STRIDE)
}

/// Encodes validated geometry with an explicit native u16 or u32 index format.
/// Static construction uses u16. Skeletal multi-size containers can use either width.
pub fn encode_with_index_width(
    vertices: &[Vertex],
    indices: &[u32],
    index_stride: usize,
) -> Result<Payload> {
    if ![size_of::<u16>(), size_of::<u32>()].contains(&index_stride) {
        return Err(failure("index width must be two or four bytes"));
    }
    if vertices.is_empty()
        || vertices.len() > i32::MAX as usize
        || (index_stride == INDEX_STRIDE && vertices.len() > MAX_COMPACT_VERTICES)
    {
        return Err(failure(
            "vertex count exceeds the selected native index format or signed array range",
        ));
    }
    if indices.is_empty()
        || !indices.len().is_multiple_of(TRIANGLE_INDICES)
        || indices.len() > i32::MAX as usize
    {
        return Err(failure("indices must be a bounded nonempty triangle list"));
    }
    let position_bytes = vertices
        .len()
        .checked_mul(POSITION_STRIDE)
        .ok_or_else(|| failure("position size overflow"))?;
    let tangent_bytes = vertices
        .len()
        .checked_mul(TANGENT_UV_STRIDE)
        .ok_or_else(|| failure("tangent size overflow"))?;
    let index_bytes = indices
        .len()
        .checked_mul(index_stride)
        .ok_or_else(|| failure("index size overflow"))?;
    let color_bytes = if vertices.iter().any(|vertex| vertex.color.is_some()) {
        vertices
            .len()
            .checked_mul(size_of::<[u8; 4]>())
            .ok_or_else(|| failure("color size overflow"))?
    } else {
        0
    };
    for vertex in vertices {
        if vertex
            .position
            .iter()
            .chain(vertex.uv.iter())
            .any(|v| !v.is_finite())
        {
            return Err(failure("position/UV must be finite"));
        }
    }
    for triangle in indices.chunks_exact(TRIANGLE_INDICES) {
        if triangle.iter().any(|&i| {
            i as usize >= vertices.len() || (index_stride == INDEX_STRIDE && i > u16::MAX as u32)
        }) {
            return Err(failure("index outside vertex range/u16 format"));
        }
    }
    let bounds = bounds(vertices)?;
    let mut positions = buffer(position_bytes)?;
    let mut tangent_uv = buffer(tangent_bytes)?;
    let mut encoded_indices = buffer(index_bytes)?;
    let mut colors = buffer(color_bytes)?;
    for vertex in vertices {
        for component in vertex.position {
            positions.extend_from_slice(&component.to_le_bytes());
        }
        if color_bytes != 0 {
            let [r, g, b, a] = vertex.color.unwrap_or([255; 4]);
            colors.extend_from_slice(&[b, g, r, a]);
        }
        let (x, z) = basis(vertex)?;
        tangent_uv.extend_from_slice(&x);
        tangent_uv.extend_from_slice(&z);
        for component in vertex.uv {
            tangent_uv.extend_from_slice(&component.to_le_bytes());
        }
    }
    for &index in indices {
        if index_stride == INDEX_STRIDE {
            encoded_indices.extend_from_slice(&(index as u16).to_le_bytes());
        } else {
            encoded_indices.extend_from_slice(&index.to_le_bytes());
        }
    }
    Ok(Payload {
        positions,
        tangent_uv,
        colors,
        indices: encoded_indices,
        index_stride,
        vertices: vertices.len(),
        triangles: indices.len() / TRIANGLE_INDICES,
        bounds,
    })
}
