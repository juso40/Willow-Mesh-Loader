//! Validated static payloads and explicit native operation dispatch.

use crate::{Error, Result, encoding, sections};

/// Validated single-LOD static payload shared by inspection and native operations.
pub struct PreparedStaticMesh {
    payload: encoding::Payload,
    sections: Vec<sections::CheckedSection>,
    material_count: usize,
}

impl PreparedStaticMesh {
    /// Encodes geometry and validates its material sections without engine access.
    pub fn new(
        vertices: &[encoding::Vertex],
        indices: &[u32],
        sections: &[sections::Section],
        material_count: usize,
    ) -> Result<Self> {
        let sections = sections::validate(indices, vertices.len(), sections, material_count)?;
        let payload = encoding::encode(vertices, indices)?;
        Ok(Self {
            payload,
            sections,
            material_count,
        })
    }

    /// Returns the encoded geometry for inspection.
    pub fn payload(&self) -> &encoding::Payload {
        &self.payload
    }

    /// Returns the already checked source-section ranges for explicit layout inspection.
    pub fn sections(&self) -> &[sections::CheckedSection] {
        &self.sections
    }

    fn check_materials(&self, materials: &[&str]) -> Result<()> {
        if materials.len() != self.material_count {
            return Err(Error("prepared static material count differs".into()));
        }
        Ok(())
    }

    /// Populates an empty Python-constructed mesh and returns its canonical path.
    /// The destination remains unattached until native initialization completes.
    pub fn populate_new(&self, path: &str, materials: &[&str]) -> Result<String> {
        self.check_materials(materials)?;
        super::population::populate_new(path, materials, &self.payload, &self.sections)
    }

    /// Replaces render geometry under the selected ownership and preservation policy.
    pub fn replace(
        &self,
        path: &str,
        materials: &[&str],
        policy: crate::ReplacementPolicy,
    ) -> Result<String> {
        self.check_materials(materials)?;
        super::replacement::replace(path, materials, &self.payload, &self.sections, policy)
    }
}
