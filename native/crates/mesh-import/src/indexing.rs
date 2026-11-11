//! Exact attribute deduplication after corner tangent generation.

use std::{collections::HashMap, hash::Hash};

use crate::{Result, Vertex, fail};

pub(crate) fn vertex_key(vertex: &Vertex) -> [u32; 22] {
    let mut key = [0; 22];
    for (out, value) in key.iter_mut().zip(
        vertex
            .position
            .into_iter()
            .chain(vertex.tangent_x)
            .chain(vertex.tangent_y)
            .chain(vertex.tangent_z)
            .chain(vertex.uv)
            .chain(vertex.extra_uvs.into_iter().flatten()),
    ) {
        *out = value.to_bits();
    }
    key[20] = u32::from_le_bytes(vertex.color.unwrap_or([255; 4]));
    key[21] = u32::from(vertex.color.is_some());
    key
}

pub(crate) fn deduplicate<V, K: Eq + Hash>(
    vertices: &mut Vec<V>,
    indices: &mut [u32],
    key: impl Fn(&V) -> K,
) -> Result<()> {
    let mut unique = Vec::new();
    let mut lookup = HashMap::new();
    let mut remap = Vec::with_capacity(vertices.len());
    for vertex in vertices.drain(..) {
        let next = unique.len();
        let index = *lookup.entry(key(&vertex)).or_insert_with(|| {
            unique.push(vertex);
            next as u32
        });
        remap.push(index);
    }
    if unique.len() > i32::MAX as usize {
        return Err(fail(
            "unique vertex count exceeds the signed native array range",
        ));
    }
    for index in indices {
        *index = remap[*index as usize];
    }
    *vertices = unique;
    Ok(())
}
