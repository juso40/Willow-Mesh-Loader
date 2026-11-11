//! Shared native storage records and component lifecycle fields.

pub(crate) const WORD_BYTES: usize = size_of::<u32>();
pub(crate) const ARRAY_HEADER_BYTES: usize = size_of::<[u32; 3]>();
pub(crate) const ARRAY_COUNT: usize = WORD_BYTES;
pub(crate) const ARRAY_CAPACITY: usize = WORD_BYTES * 2;
pub(crate) const COMPONENT_SCENE: usize = 0x4c;
pub(crate) const COMPONENT_FLAGS: usize = 0x54;
pub(crate) const COMPONENT_ATTACHED: u8 = 1;
pub(crate) type ReattachContext = [u32; 3];

pub(crate) mod vertex_impl {
    pub(crate) const SIZE: usize = 24;
    pub(crate) const ARRAY: usize = 8;
}
