//! Explicit importer-to-runtime data conversion without Python marshaling.

pub(crate) fn vertex(vertex: &mesh_import::Vertex) -> mesh_runtime::encoding::Vertex {
    mesh_runtime::encoding::Vertex {
        position: vertex.position,
        tangent_x: vertex.tangent_x,
        tangent_y: vertex.tangent_y,
        tangent_z: vertex.tangent_z,
        uv: vertex.uv,
        color: vertex.color,
    }
}

pub(crate) fn bone(bone: mesh_import::skin::ImportBone) -> mesh_runtime::skeletal::ReferenceBone {
    mesh_runtime::skeletal::ReferenceBone {
        name: bone.name,
        parent: bone.parent,
        translation: bone.translation,
        rotation_xyzw: bone.rotation_xyzw,
    }
}

pub(crate) fn sections(sections: &[mesh_import::Section]) -> Vec<mesh_runtime::sections::Section> {
    sections
        .iter()
        .enumerate()
        .map(
            |(material_index, section)| mesh_runtime::sections::Section {
                first_index: section.first_index,
                triangles: section.triangles,
                material_index,
            },
        )
        .collect()
}

pub(crate) fn clip(
    clip: mesh_import::animation::AnimationClip,
) -> mesh_runtime::animation::AnimationClip {
    mesh_runtime::animation::AnimationClip {
        name: clip.name,
        duration: clip.duration,
        frames: clip.frames,
        tracks: clip
            .tracks
            .into_iter()
            .map(|track| mesh_runtime::animation::BoneTrack {
                translations: track.translations,
                rotations: track.rotations,
            })
            .collect(),
    }
}
