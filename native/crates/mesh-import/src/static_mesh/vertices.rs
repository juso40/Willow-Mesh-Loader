//! Static primitive assembly using the shared vertex conversion path.

use crate::math::Matrix;
use crate::{Geometry, Result, Section, fail, primitive};

pub(super) fn append(
    primitive: gltf::Primitive<'_>,
    transform: Matrix,
    buffers: &crate::document::Buffers<'_>,
    out: &mut Geometry,
) -> Result<()> {
    let converted = primitive::convert(
        primitive.clone(),
        transform,
        buffers,
        out.vertices.len(),
        &mut out.primitives,
        &mut out.warnings,
        false,
    )?;
    let first_index = out.indices.len();
    let vertices = converted.vertices.len();
    out.vertices
        .try_reserve(vertices)
        .map_err(|_| fail("host vertex allocation failed"))?;
    out.indices
        .try_reserve(vertices)
        .map_err(|_| fail("host index allocation failed"))?;
    out.indices
        .extend((out.vertices.len()..out.vertices.len() + vertices).map(|index| index as u32));
    out.vertices.extend(converted.vertices);
    out.sections.push(Section {
        first_index,
        triangles: vertices / 3,
        source_material: primitive.material().index(),
    });
    Ok(())
}
