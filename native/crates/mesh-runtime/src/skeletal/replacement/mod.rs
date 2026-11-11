//! Geometry-only skeletal replacement with preserved reference data and expanded bounds.

use crate::{Error, Result, encoding::Bounds};

fn expanded_bounds(old: Bounds, incoming: Bounds) -> Result<Bounds> {
    if old.origin.iter().any(|value| !value.is_finite())
        || old
            .box_extent
            .iter()
            .chain([&old.sphere_radius])
            .any(|value| !value.is_finite() || *value < 0.0)
    {
        return Err(Error(
            "skeletal reload refuses invalid existing bounds".into(),
        ));
    }
    let mut bounds = old;
    let mut distance_squared = 0.0f64;
    for index in 0..3 {
        let delta = f64::from(incoming.origin[index]) - f64::from(old.origin[index]);
        distance_squared += delta * delta;
        let extent = delta.abs() + f64::from(incoming.box_extent[index]);
        bounds.box_extent[index] = old.box_extent[index].max((extent as f32).next_up());
    }
    bounds.sphere_radius = old
        .sphere_radius
        .max(((distance_squared.sqrt() + f64::from(incoming.sphere_radius)) as f32).next_up());
    if bounds
        .box_extent
        .iter()
        .chain([&bounds.sphere_radius])
        .any(|value| !value.is_finite())
    {
        return Err(Error(
            "expanded skeletal bounds exceed finite f32 range".into(),
        ));
    }
    Ok(bounds)
}

mod runtime;
pub(super) use runtime::{
    bind_template, mark_gestalt, register, replace, replace_gestalt, update_gestalt,
};
