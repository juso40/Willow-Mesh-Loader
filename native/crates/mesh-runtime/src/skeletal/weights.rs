//! Palette mapping and deterministic byte-weight quantization.

use super::{MAX_PALETTE_BONES, SkinWeights, failure};
use crate::Result;

/// Four palette-local byte indices/weights. Constructed only by validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Influences {
    bones: [u8; 4],
    weights: [u8; 4],
}

impl Influences {
    /// Normalizes finite nonnegative inputs and quantizes to exactly 255 with
    /// largest-remainder rounding. Ties use input-slot order. No influence
    /// reduction, duplicate merging or silent invalid-joint repair.
    pub fn quantize(bones: [u8; 4], weights: [f32; 4], palette_len: usize) -> Result<Self> {
        if !(1..=MAX_PALETTE_BONES).contains(&palette_len)
            || bones.iter().any(|&bone| usize::from(bone) >= palette_len)
        {
            return Err(failure("skin influence index exceeds the chunk palette"));
        }
        if weights.iter().any(|&w| !w.is_finite() || w < 0.) {
            return Err(failure("skin weights must be finite and nonnegative"));
        }
        for a in 0..4 {
            for b in a + 1..4 {
                if weights[a] > 0. && weights[b] > 0. && bones[a] == bones[b] {
                    return Err(failure("duplicate nonzero skin influence"));
                }
            }
        }
        // f64 sum prevents overflow for four otherwise-valid large f32 weights.
        let sum: f64 = weights.iter().map(|&w| f64::from(w)).sum();
        if sum == 0. {
            return Err(failure("skin weights must have a nonzero sum"));
        }
        let scaled = weights.map(|w| f64::from(w) / sum * f64::from(u8::MAX));
        let mut bytes = scaled.map(|w| w.floor() as u8);
        let assigned: u16 = bytes.iter().map(|&w| u16::from(w)).sum();
        let remaining = u16::from(u8::MAX)
            .checked_sub(assigned)
            .ok_or_else(|| failure("skin weight quantization overflow"))?;
        if usize::from(remaining) > weights.len() {
            return Err(failure("skin weight quantization remainder exceeded"));
        }
        let mut order = [0, 1, 2, 3];
        order.sort_by(|&a, &b| {
            let fa = scaled[a] - f64::from(bytes[a]);
            let fb = scaled[b] - f64::from(bytes[b]);
            fb.total_cmp(&fa).then(a.cmp(&b))
        });
        for &slot in &order[..usize::from(remaining)] {
            bytes[slot] = bytes[slot]
                .checked_add(1)
                .ok_or_else(|| failure("skin weight quantization overflow"))?;
        }
        // Consumers may fetch all four bones even for zero weights. Keep every
        // byte in range, and canonicalize inactive slots to palette entry zero.
        let mut canonical = bones;
        for i in 0..4 {
            if bytes[i] == 0 {
                canonical[i] = 0;
            }
        }
        Ok(Self {
            bones: canonical,
            weights: bytes,
        })
    }

    /// Maps global influences through one checked palette and quantizes every vertex.
    /// The palette contains every weighted joint exactly once and is validated once.
    pub fn quantize_global(influences: &[SkinWeights], palette: &[u16]) -> Result<Vec<Self>> {
        if palette.is_empty() || palette.len() > MAX_PALETTE_BONES {
            return Err(failure("chunk palette exceeds the GPU bone budget"));
        }
        for a in 0..palette.len() {
            for b in a + 1..palette.len() {
                if palette[a] == palette[b] {
                    return Err(failure("chunk palette contains duplicate bones"));
                }
            }
        }
        influences
            .iter()
            .map(|influence| {
                let mut local = [0u8; 4];
                for (slot, local) in local.iter_mut().enumerate() {
                    if influence.weights[slot] == 0. {
                        // Zero-weight joints need not appear in the palette.
                        continue;
                    }
                    let position = palette
                        .iter()
                        .position(|&joint| joint == influence.joints[slot])
                        .ok_or_else(|| {
                            failure("skin influence joint is absent from the chunk palette")
                        })?;
                    *local = position as u8;
                }
                Self::quantize(local, influence.weights, palette.len())
            })
            .collect()
    }

    /// Returns chunk-local indices rather than global skeleton indices.
    pub fn bones(&self) -> [u8; 4] {
        self.bones
    }

    /// Returns normalized byte weights summing to 255.
    pub fn weights(&self) -> [u8; 4] {
        self.weights
    }
}
