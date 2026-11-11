//! Static geometry loading with only the selected node's ancestor transforms.

mod vertices;

use crate::{Geometry, Result, Source, fail, hierarchy::Hierarchy, math::*};

pub(crate) fn from_source(source: &Source, scale: f32) -> Result<Geometry> {
    crate::policy::validate_scale(scale)?;
    let node = source.node()?;
    if node.skin().is_some() {
        return Err(fail("skinned nodes are not supported by static import"));
    }
    let hierarchy = Hierarchy::read(source)?;
    let worlds = hierarchy.worlds([node.index()])?;
    let world = worlds[node.index()]
        .ok_or_else(|| fail("selected mesh node is not reachable from the selected scene"))?;
    let transform = checked_matrix(matrix_mul(
        crate::coordinates::matrix(f64::from(scale)),
        world,
    ))?;
    let buffers = source.mesh_buffers()?;
    let mut out = Geometry {
        vertices: Vec::new(),
        indices: Vec::new(),
        scene: source.scene()?.index(),
        instances: 1,
        primitives: 0,
        sections: Vec::new(),
        warnings: Vec::new(),
    };
    let mesh = node
        .mesh()
        .ok_or_else(|| fail("selected node has no mesh"))?;
    for primitive in mesh.primitives() {
        vertices::append(primitive, transform, &buffers, &mut out)?;
    }
    if out.indices.is_empty() {
        return Err(fail(
            "selected scene contains no nonempty triangle geometry",
        ));
    }
    crate::indexing::deduplicate(
        &mut out.vertices,
        &mut out.indices,
        crate::indexing::vertex_key,
    )?;
    (out.indices, out.sections) = crate::sections::group(&out.indices, &out.sections)?;
    Ok(out)
}
