//! Classification of explicitly selected mesh nodes.

/// Native mesh class required by a selected node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Static geometry with baked node transforms.
    Static,
    /// Skinned geometry with its bind-derived skeleton.
    Skeletal,
}
