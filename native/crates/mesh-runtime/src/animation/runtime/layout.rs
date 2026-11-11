//! Reviewed BL2 x86 animation object offsets and flag masks.

pub(super) mod sequence {
    pub(crate) const CLASS: &str = "Engine.AnimSequence";
    pub(crate) const SIZE: usize = 0x104;
    pub(crate) const NAME: usize = 0x3c;
    pub(crate) const NOTIFIES: usize = 0x44;
    pub(crate) const LENGTH: usize = 0x68;
    pub(crate) const FRAMES: usize = 0x6c;
    pub(crate) const RATE_SCALE: usize = 0x70;
    pub(crate) const FLAGS: usize = 0x74;
    pub(crate) const NO_LOOPING_INTERPOLATION: u32 = 1;
    pub(crate) const ADDITIVE: u32 = 2;
    pub(crate) const RAW: usize = 0x84;
    pub(crate) const TRANSLATIONS: usize = 0x90;
    pub(crate) const ROTATIONS: usize = 0x9c;
    pub(crate) const CURVES: usize = 0xa8;
    pub(crate) const TRANSLATION_FORMAT: usize = 0xb4;
    pub(crate) const ROTATION_FORMAT: usize = 0xb5;
    pub(crate) const KEY_FORMAT: usize = 0xb6;
    pub(crate) const OFFSETS: usize = 0xb8;
    pub(crate) const STREAM: usize = 0xc4;
    pub(crate) const TRANSLATION_CODEC: usize = 0xd0;
    pub(crate) const ROTATION_CODEC: usize = 0xd4;
    pub(crate) const DELTA_CACHE: usize = 0xf8;
    pub(crate) const RAW_TRACK_SIZE: usize = 24;
    pub(crate) const RAW_ROTATIONS: usize = 12;
    pub(crate) const CONSTANT_FLOAT96_FORMATS: [u8; 3] = [1, 1, 0];
    pub(crate) const FRESH_ARRAYS: &[usize] = &[
        RAW,
        TRANSLATIONS,
        ROTATIONS,
        CURVES,
        OFFSETS,
        STREAM,
        DELTA_CACHE,
        NOTIFIES,
    ];
}

pub(super) mod anim_set {
    pub(crate) const CLASS: &str = "Engine.AnimSet";
    pub(crate) const SIZE: usize = 0x11c;
    pub(crate) const FLAGS: usize = 0x3c;
    pub(crate) const ROTATION_ONLY: u32 = 1;
    pub(crate) const TRACK_NAMES: usize = 0x40;
    pub(crate) const SEQUENCES: usize = 0x4c;
    pub(crate) const SEQUENCE_CACHE: usize = 0x58;
    pub(crate) const LINKUPS: usize = 0x94;
    pub(crate) const FRESH_ARRAYS: &[usize] = &[TRACK_NAMES, SEQUENCES, SEQUENCE_CACHE, LINKUPS];
}
