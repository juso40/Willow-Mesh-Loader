//! Validated skeletal payloads and explicit native operation dispatch.

use crate::{
    Error, Result,
    sections::{self, CheckedSection},
    skeletal::{self, ReferenceBone, SkinWeights, SkinnedMesh},
};

/// Engine-space geometry used to prepare a skeletal payload.
#[derive(Debug, Clone, Copy)]
pub struct SkinnedGeometryInput<'a> {
    /// Converted vertices with orthonormal tangent bases.
    pub vertices: &'a [crate::encoding::Vertex],
    /// Triangle-list indices partitioned by the sections.
    pub indices: &'a [u32],
    /// Per-vertex influences using global skeleton joints.
    pub weights: &'a [SkinWeights],
    /// Additional UV channels after vertex UV0.
    pub extra_uvs: &'a [Vec<[f32; 2]>],
    /// Parent-before-child reference bones with a single root.
    pub skeleton: &'a [ReferenceBone],
    /// Index ranges referencing component material-slot ordinals.
    pub sections: &'a [sections::Section],
}

/// Validated skeletal payload shared by inspection and native operations.
#[derive(Clone)]
pub struct PreparedSkeletalMesh {
    mesh: SkinnedMesh,
    sections: Vec<CheckedSection>,
    material_count: usize,
}

impl PreparedSkeletalMesh {
    /// Encodes skinning and validates material sections without engine access.
    pub fn new(geometry: &SkinnedGeometryInput<'_>, material_count: usize) -> Result<Self> {
        if geometry
            .sections
            .iter()
            .any(|section| section.material_index > usize::from(u16::MAX))
        {
            return Err(Error(
                "referenced skeletal material ordinal exceeds the native u16 range".into(),
            ));
        }
        let sections = sections::validate(
            geometry.indices,
            geometry.vertices.len(),
            geometry.sections,
            material_count,
        )?;
        let mesh = skeletal::skinned_with_uvs(
            geometry.vertices,
            geometry.indices,
            geometry.weights,
            geometry.skeleton,
            geometry.extra_uvs,
        )?;
        Ok(Self {
            mesh,
            sections,
            material_count,
        })
    }

    pub(super) fn encoded(
        mesh: SkinnedMesh,
        sections: Vec<CheckedSection>,
        material_count: usize,
    ) -> Self {
        Self {
            mesh,
            sections,
            material_count,
        }
    }

    /// Aligns compatible joint ordinals without imposing the template's UV or color layout.
    pub fn bind_skeleton(&mut self, template: &[ReferenceBone]) -> Result<()> {
        super::bind_to_template(&mut self.mesh, template)
    }

    /// Concatenates compatible geometry with fresh incoming material slots and stable old ranges.
    pub fn concatenate(self, right: &Self) -> Result<Self> {
        let left_sections = self.section_inputs();
        let right_sections = right.section_inputs();
        let (mesh, sections) = super::gestalt::concatenate(
            self.mesh,
            &right.mesh,
            &left_sections,
            &right_sections,
            self.material_count,
            right.material_count,
        )?;
        Ok(Self::encoded(
            mesh,
            sections,
            self.material_count + right.material_count,
        ))
    }

    pub(super) fn section_inputs(&self) -> Vec<sections::Section> {
        self.sections
            .iter()
            .map(|section| sections::Section {
                first_index: section.first_index as usize,
                triangles: section.triangles as usize,
                material_index: section.material_index as usize,
            })
            .collect()
    }

    /// Replaces a Gestalt master through the dedicated geometry path, preserving its skeleton.
    pub fn replace_gestalt(&self, path: &str, materials: &[&str]) -> Result<String> {
        self.check_materials(materials)?;
        let mut incoming = self.clone();
        incoming.bind_skeleton(&super::get_reference_skeleton(path)?)?;
        super::replacement::replace_gestalt(path, materials, &incoming.mesh, &incoming.sections)
    }

    /// Appends to current native geometry and returns index and material offsets for new rows.
    /// Existing material ordinals and index ranges remain unchanged.
    pub fn append_gestalt(&self, path: &str, materials: &[&str]) -> Result<(String, usize, usize)> {
        self.check_materials(materials)?;
        let (current, mut paths) = super::readback::read_current(path)?;
        let first_index = current.mesh.skin.triangles * crate::encoding::TRIANGLE_INDICES;
        let material_offset = paths.len();
        let mut incoming = self.clone();
        incoming.bind_skeleton(&current.mesh.skeleton)?;
        let combined = current.concatenate(&incoming)?;
        paths.extend(materials.iter().map(|path| (*path).to_owned()));
        let references = paths.iter().map(String::as_str).collect::<Vec<_>>();
        let canonical = combined.replace_gestalt(path, &references)?;
        Ok((canonical, first_index, material_offset))
    }

    pub(super) fn sections(&self) -> &[CheckedSection] {
        &self.sections
    }

    pub(super) fn material_count(&self) -> usize {
        self.material_count
    }

    /// Replaces existing named Gestalt fragments and refreshes current component selections.
    /// Material assignments remain unchanged unless preservation is disabled.
    pub fn update_gestalt(
        &self,
        definition: &str,
        parts: &[super::GestaltPartRange],
        materials: &[&str],
        preserve_materials: bool,
    ) -> Result<String> {
        self.check_materials(materials)?;
        super::gestalt_update::update(self, definition, parts, materials, preserve_materials)
    }

    /// Returns the prepared skin and skeleton for inspection.
    pub fn mesh(&self) -> &SkinnedMesh {
        &self.mesh
    }

    fn check_materials(&self, materials: &[&str]) -> Result<()> {
        if materials.len() != self.material_count {
            return Err(Error("prepared skeletal material count differs".into()));
        }
        Ok(())
    }

    /// Populates an empty Python-constructed mesh and returns its canonical path.
    /// The destination remains unattached until native initialization completes.
    pub fn populate_new(&self, path: &str, materials: &[&str]) -> Result<String> {
        self.check_materials(materials)?;
        super::population::populate_new(path, materials, &self.mesh, &self.sections)
    }

    /// Binds the prepared skeleton to an existing Gestalt template before object creation.
    pub fn bind_template(&mut self, template: &str) -> Result<()> {
        super::replacement::bind_template(template, &mut self.mesh)
    }

    /// Populates an empty mesh and registers its Gestalt ownership.
    pub fn populate_gestalt(&self, path: &str, materials: &[&str]) -> Result<String> {
        let path = self.populate_new(path, materials)?;
        super::replacement::mark_gestalt(&path)?;
        Ok(path)
    }

    /// Replaces geometry under the selected ownership policy with unchanged skeleton and caches.
    pub fn replace(
        &self,
        path: &str,
        materials: &[&str],
        policy: crate::ReplacementPolicy,
    ) -> Result<String> {
        self.check_materials(materials)?;
        super::replacement::replace(path, materials, &self.mesh, &self.sections, policy)
    }
}
