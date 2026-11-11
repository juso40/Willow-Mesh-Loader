//! Checked glTF animation channels and interpolation in source coordinates.

use crate::{Result, document, fail};
use gltf::{
    accessor::{DataType, Dimensions},
    animation::{Interpolation, Property, util::ReadOutputs},
};

pub(super) struct Channel {
    pub node: usize,
    pub property: Property,
    interpolation: Interpolation,
    times: Vec<f64>,
    values: Vec<[f64; 4]>,
}

impl Channel {
    pub fn read(
        channel: gltf::animation::Channel<'_>,
        buffers: &crate::document::Buffers<'_>,
    ) -> Result<Self> {
        let sampler = channel.sampler();
        let property = channel.target().property();
        if property == Property::MorphTargetWeights {
            return Err(fail("animation morph-weight channels are not supported"));
        }
        document::typed(
            &sampler.output(),
            DataType::F32,
            if property == Property::Rotation {
                Dimensions::Vec4
            } else {
                Dimensions::Vec3
            },
        )?;
        let reader = channel.reader(|buffer| buffers.get(buffer.index()));
        let times = Self::read_times(&channel, buffers)?;
        let values = match reader
            .read_outputs()
            .ok_or_else(|| fail("animation values unavailable"))?
        {
            ReadOutputs::Translations(values) | ReadOutputs::Scales(values) => values
                .map(|v| [f64::from(v[0]), f64::from(v[1]), f64::from(v[2]), 0.])
                .collect::<Vec<_>>(),
            ReadOutputs::Rotations(values) => values.into_f32().map(|v| v.map(f64::from)).collect(),
            _ => return Err(fail("animation output does not match its target")),
        };
        let interpolation = sampler.interpolation();
        let factor = if interpolation == Interpolation::CubicSpline {
            3
        } else {
            1
        };
        if times.len().checked_mul(factor) != Some(values.len())
            || values.iter().flatten().any(|v| !v.is_finite())
        {
            return Err(fail("animation output count or values are invalid"));
        }
        if property == Property::Rotation {
            for index in 0..times.len() {
                let value = values[index * factor + usize::from(factor == 3)];
                let norm = value.iter().map(|v| v * v).sum::<f64>();
                if (norm - 1.).abs() > crate::policy::QUATERNION_NORM_TOLERANCE {
                    return Err(fail("animation rotation keys must be unit quaternions"));
                }
            }
        }
        Ok(Self {
            node: channel.target().node().index(),
            property,
            interpolation,
            times,
            values,
        })
    }

    pub fn read_times(
        channel: &gltf::animation::Channel<'_>,
        buffers: &crate::document::Buffers<'_>,
    ) -> Result<Vec<f64>> {
        document::typed(
            &channel.sampler().input(),
            DataType::F32,
            Dimensions::Scalar,
        )?;
        let reader = channel.reader(|buffer| buffers.get(buffer.index()));
        let times: Vec<_> = reader
            .read_inputs()
            .ok_or_else(|| fail("animation times unavailable"))?
            .map(f64::from)
            .collect();
        if times.is_empty()
            || times.iter().any(|t| !t.is_finite() || *t < 0.)
            || times.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(fail(
                "animation times must be finite, nonnegative and strictly increasing",
            ));
        }
        Ok(times)
    }

    pub fn end(&self) -> f64 {
        self.times[self.times.len() - 1]
    }

    fn key(&self, index: usize) -> [f64; 4] {
        self.values[if self.interpolation == Interpolation::CubicSpline {
            index * 3 + 1
        } else {
            index
        }]
    }

    pub fn sample(&self, time: f64) -> Result<[f64; 4]> {
        let upper = self.times.partition_point(|t| *t <= time);
        let mut value = if upper == 0 {
            self.key(0)
        } else if upper == self.times.len() || self.interpolation == Interpolation::Step {
            self.key(upper - 1)
        } else {
            let lower = upper - 1;
            let duration = self.times[upper] - self.times[lower];
            let alpha = (time - self.times[lower]) / duration;
            let a = self.key(lower);
            let mut b = self.key(upper);
            if self.interpolation == Interpolation::CubicSpline {
                let t2 = alpha * alpha;
                let t3 = t2 * alpha;
                std::array::from_fn(|i| {
                    (2. * t3 - 3. * t2 + 1.) * a[i]
                        + (t3 - 2. * t2 + alpha) * duration * self.values[lower * 3 + 2][i]
                        + (-2. * t3 + 3. * t2) * b[i]
                        + (t3 - t2) * duration * self.values[upper * 3][i]
                })
            } else if self.property == Property::Rotation {
                let mut dot = a.iter().zip(b).map(|(a, b)| a * b).sum::<f64>();
                if dot < 0. {
                    b = b.map(|v| -v);
                    dot = -dot;
                }
                if dot > 0.9995 {
                    std::array::from_fn(|i| a[i] + alpha * (b[i] - a[i]))
                } else {
                    let angle = dot.clamp(-1., 1.).acos();
                    let denominator = angle.sin();
                    std::array::from_fn(|i| {
                        (((1. - alpha) * angle).sin() * a[i] + (alpha * angle).sin() * b[i])
                            / denominator
                    })
                }
            } else {
                std::array::from_fn(|i| a[i] + alpha * (b[i] - a[i]))
            }
        };
        if self.property == Property::Rotation {
            let norm = value.iter().map(|v| v * v).sum::<f64>().sqrt();
            if !norm.is_finite() || norm < crate::policy::MIN_QUATERNION_NORM {
                return Err(fail("sampled animation rotation is degenerate"));
            }
            value = value.map(|v| v / norm);
        }
        if value.iter().any(|v| !v.is_finite()) {
            return Err(fail("sampled animation values must be finite"));
        }
        Ok(value)
    }
}
