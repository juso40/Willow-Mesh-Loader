//! Native codec selection and AnimSet lookup-cache initialization signatures.

use crate::engine::native::{self, AnimationCodecLinks, Method, NativeFunction};

pub(super) static CODEC_LINKS: NativeFunction<AnimationCodecLinks> = NativeFunction::new(
    "AnimationFormatSetInterfaceLinks",
    "55 8B EC 56 8B 75 08 8A 86 B6 00 00 00 33 C9 89 8E D0 00 00 00 89 8E D4 00 00 00",
    &[],
);
pub(super) static SET_POST_LOAD: NativeFunction<Method<native::AnimSet>> = NativeFunction::new(
    "AnimSetPostLoad",
    "55 8B EC 6A FF 68 ?? ?? ?? ?? 64 A1 00 00 00 00 50 83 EC 10 53 56 57 A1 ?? ?? ?? ?? 33 C5 50 8D 45 F4 64 A3 00 00 00 00 8B F1 A1 ?? ?? ?? ?? 64 8B 0D 2C 00 00 00 8B 1C 81 8B 83 08 00 00 00 C6 84 18 0C 00 00 00 14 FF 83 08 00 00 00 89 45 F0 33 FF 8B CE 89 7D FC E8 ?? ?? ?? ?? 39 7E 50",
    &[],
);
