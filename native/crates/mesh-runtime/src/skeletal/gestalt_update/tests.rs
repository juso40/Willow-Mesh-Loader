//! Numerical and packed-layout invariants for named range replacement.

use super::{PartRange, geometry};
use crate::{
    encoding::Vertex,
    sections::Section,
    skeletal::{PreparedSkeletalMesh, ReferenceBone, SkinWeights, SkinnedGeometryInput, records},
};

fn part(name: &str, first_index: usize, triangles: usize, material_slot: usize) -> PartRange {
    PartRange {
        name: name.into(),
        first_index,
        triangles,
        material_slot,
    }
}

fn mesh(
    triangles: usize,
    offset: f32,
    materials: usize,
    channels: usize,
    colors: bool,
    bones: usize,
) -> PreparedSkeletalMesh {
    let vertices: Vec<_> = (0..triangles * 3)
        .map(|index| Vertex {
            position: [
                offset + (index / 3) as f32 * 2.0 + f32::from(index % 3 == 1),
                f32::from(index % 3 == 2),
                0.0,
            ],
            tangent_x: [1., 0., 0.],
            tangent_y: [0., 1., 0.],
            tangent_z: [0., 0., 1.],
            uv: [0.25, 0.75],
            color: colors.then_some([index as u8, 32, 64, 255]),
        })
        .collect();
    let indices: Vec<_> = (0..vertices.len() as u32).collect();
    let weights: Vec<_> = (0..vertices.len())
        .map(|index| SkinWeights {
            joints: [index.min(bones - 1) as u16, 0, 0, 0],
            weights: [1., 0., 0., 0.],
        })
        .collect();
    let skeleton: Vec<_> = (0..bones)
        .map(|index| ReferenceBone {
            name: format!("bone{index}"),
            parent: (index != 0).then_some(0),
            translation: [0.; 3],
            rotation_xyzw: [0., 0., 0., 1.],
        })
        .collect();
    let extra_uvs: Vec<_> = (1..channels)
        .map(|channel| vec![[channel as f32, 0.5]; vertices.len()])
        .collect();
    let sections: Vec<_> = (0..materials)
        .map(|slot| Section {
            first_index: slot * 3,
            triangles: if slot + 1 == materials {
                triangles - slot
            } else {
                1
            },
            material_index: slot,
        })
        .collect();
    PreparedSkeletalMesh::new(
        &SkinnedGeometryInput {
            vertices: &vertices,
            indices: &indices,
            weights: &weights,
            extra_uvs: &extra_uvs,
            skeleton: &skeleton,
            sections: &sections,
        },
        materials,
    )
    .expect("valid encoded fixture")
}

#[test]
fn preserves_aliased_triangles_and_packed_streams_while_padding_new_uvs_and_colors() {
    let current = mesh(2, 0., 1, 2, true, 1);
    let incoming = mesh(2, 100., 1, 1, false, 1);
    let existing = [
        part("BodyBandit", 0, 1, 0),
        part("BodyAlias", 0, 1, 0),
        part("Grip", 3, 1, 0),
    ];
    let replacements = [part("BodyBandit", 0, 2, 0)];
    let updated = geometry::update(&current, &existing, &incoming, &replacements, true)
        .expect("valid aliased update");
    assert_eq!(
        updated.parts,
        [
            part("BodyAlias", 0, 1, 0),
            part("Grip", 3, 1, 0),
            part("BodyBandit", 6, 2, 0)
        ]
    );
    let skin = &updated.mesh.mesh().skin;
    assert_eq!(skin.triangles, 4);
    assert_eq!(skin.tex_coords, 2);
    assert_eq!(
        &skin.gpu_vertices[..current.mesh().skin.gpu_vertices.len()],
        current.mesh().skin.gpu_vertices
    );
    assert_eq!(&skin.colors[..24], current.mesh().skin.colors);
    assert!(skin.colors[24..].iter().all(|&value| value == 255));
    for vertex in skin.gpu_vertices.chunks_exact(skin.vertex_stride()).skip(6) {
        assert_eq!(&vertex[36..44], &[0; 8]);
    }
    assert_eq!(updated.bounds["BodyBandit"].origin, [101.5, 0.5, 0.]);
}

#[test]
fn compacts_removed_vertices_and_keeps_repeated_updates_stable() {
    let current = mesh(2, 0., 1, 1, true, 1);
    let incoming = mesh(2, 100., 1, 1, true, 1);
    let existing = [part("BodyBandit", 0, 1, 0), part("Grip", 3, 1, 0)];
    let replacements = [part("BodyBandit", 0, 2, 0)];
    let first = geometry::update(&current, &existing, &incoming, &replacements, true)
        .expect("valid compacting update");
    assert_eq!(
        first.parts,
        [part("Grip", 0, 1, 0), part("BodyBandit", 3, 2, 0)]
    );
    assert_eq!(first.mesh.mesh().skin.vertices, 9);
    let stride = current.mesh().skin.vertex_stride();
    assert_eq!(
        &first.mesh.mesh().skin.gpu_vertices[..3 * stride],
        &current.mesh().skin.gpu_vertices[3 * stride..]
    );
    let second = geometry::update(&first.mesh, &first.parts, &incoming, &replacements, true)
        .expect("valid repeated update");
    assert_eq!(second.parts, first.parts);
    assert_eq!(
        second.mesh.mesh().skin.gpu_vertices,
        first.mesh.mesh().skin.gpu_vertices
    );
    assert_eq!(
        second.mesh.mesh().skin.indices,
        first.mesh.mesh().skin.indices
    );
    assert_eq!(
        second.mesh.mesh().skin.colors,
        first.mesh.mesh().skin.colors
    );
}

#[test]
fn retains_partially_aliased_and_unclaimed_triangles_with_rebased_ranges() {
    let current = mesh(3, 0., 1, 1, false, 1);
    let incoming = mesh(1, 100., 1, 1, false, 1);
    let existing = [part("BodyBandit", 0, 2, 0), part("Alias", 3, 1, 0)];
    let updated = geometry::update(
        &current,
        &existing,
        &incoming,
        &[part("BodyBandit", 0, 1, 0)],
        true,
    )
    .expect("valid partially aliased update");
    assert_eq!(
        updated.parts,
        [part("Alias", 0, 1, 0), part("BodyBandit", 6, 1, 0)]
    );
    assert_eq!(updated.mesh.mesh().skin.triangles, 3);
    let stride = current.mesh().skin.vertex_stride();
    assert_eq!(
        &updated.mesh.mesh().skin.gpu_vertices[..6 * stride],
        &current.mesh().skin.gpu_vertices[3 * stride..]
    );
}

#[test]
fn fills_named_empty_slots_without_discarding_existing_zero_length_metadata() {
    let current = mesh(1, 0., 1, 1, false, 1);
    let incoming = mesh(1, 100., 1, 1, false, 1);
    let existing = [
        part("Body", 0, 1, 0),
        part("Empty", 0, 0, 0),
        part("None", 3, 0, 0),
    ];
    let updated = geometry::update(
        &current,
        &existing,
        &incoming,
        &[part("Empty", 0, 1, 0)],
        true,
    )
    .expect("valid named empty slot update");
    assert_eq!(
        updated.parts,
        [
            part("Body", 0, 1, 0),
            part("None", 3, 0, 0),
            part("Empty", 3, 1, 0)
        ]
    );
    assert_eq!(updated.mesh.mesh().skin.triangles, 2);
    assert!(
        geometry::update(
            &current,
            &existing,
            &incoming,
            &[part("None", 0, 1, 0)],
            true
        )
        .is_err()
    );
}

#[test]
fn retains_material_groups_or_assigns_fresh_slots_without_old_geometry() {
    let current = mesh(2, 0., 2, 3, true, 1);
    let incoming = mesh(1, 100., 1, 1, false, 1);
    let existing = [part("BodyBandit", 0, 1, 0), part("BodyBandit", 3, 1, 1)];
    let replacements = [part("BodyBandit", 0, 1, 0)];
    assert!(geometry::update(&current, &existing, &incoming, &replacements, true).is_err());
    let updated = geometry::update(&current, &existing, &incoming, &replacements, false)
        .expect("valid full geometry update");
    assert_eq!(updated.parts, [part("BodyBandit", 0, 1, 2)]);
    assert_eq!(updated.mesh.material_count(), 3);
    assert_eq!(updated.mesh.mesh().skin.triangles, 1);
    assert_eq!(updated.mesh.mesh().skin.tex_coords, 3);
    assert_eq!(updated.mesh.mesh().skin.colors, [255; 12]);
    for vertex in updated.mesh.mesh().skin.gpu_vertices.chunks_exact(52) {
        assert_eq!(&vertex[36..], &[0; 16]);
    }
    let source = mesh(2, 100., 2, 1, false, 1);
    let updated = geometry::update(&current, &existing, &source, &existing, true)
        .expect("matching material groups");
    assert_eq!(updated.parts, existing);
    assert_eq!(updated.mesh.material_count(), 2);
}

#[test]
fn rejects_unknown_identities_bad_ranges_and_changed_reference_frames() {
    let current = mesh(1, 0., 1, 1, false, 1);
    let incoming = mesh(1, 100., 1, 1, false, 1);
    let existing = [part("BodyBandit", 0, 1, 0)];
    for replacement in [
        part("Unknown", 0, 1, 0),
        part("bodybandit", 0, 1, 0),
        part("BodyBandit", 1, 1, 0),
        part("BodyBandit", 0, 2, 0),
        part("BodyBandit", 0, 1, 1),
    ] {
        assert!(geometry::update(&current, &existing, &incoming, &[replacement], true).is_err());
    }
    let mut source = incoming.mesh().clone();
    source.skeleton[0].translation[0] = 5.;
    let incoming = PreparedSkeletalMesh::encoded(source, incoming.sections().to_vec(), 1);
    assert!(geometry::update(&current, &existing, &incoming, &existing, true).is_err());
}

#[test]
fn preserves_chunk_local_weight_bytes_across_compaction_and_native_staging() {
    let current = mesh(27, 0., 1, 1, false, 80);
    let incoming = mesh(1, 100., 1, 1, false, 80);
    assert_eq!(current.mesh().skin.chunks.len(), 2);
    let existing = [part("BodyBandit", 0, 1, 0), part("Other", 3, 26, 0)];
    let updated = geometry::update(
        &current,
        &existing,
        &incoming,
        &[part("BodyBandit", 0, 1, 0)],
        true,
    )
    .expect("valid chunked update");
    let skin = &updated.mesh.mesh().skin;
    assert_eq!(skin.chunks.len(), 3);
    assert_eq!(
        skin.chunks[0].palette,
        current.mesh().skin.chunks[0].palette
    );
    assert_eq!(
        skin.chunks[1].palette,
        current.mesh().skin.chunks[1].palette
    );
    let stride = skin.vertex_stride();
    assert_eq!(
        &skin.gpu_vertices[..78 * stride],
        &current.mesh().skin.gpu_vertices[3 * stride..]
    );
    let names: Vec<_> = (0..80)
        .map(|index| records::NameWords {
            index: index + 1,
            number: 0,
        })
        .collect();
    records::stage(updated.mesh.mesh(), &names, updated.mesh.sections())
        .expect("valid native chunk and section records");
}
