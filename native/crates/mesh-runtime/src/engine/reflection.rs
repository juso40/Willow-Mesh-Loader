//! SDK lookup of loaded objects for ownership-checked native mutation.

use super::construct;
use crate::{Error, Result};
use unrealsdk_rs::{objects::find_object, types::UObject};

pub(crate) fn loaded_object(path: &str, class_name: &str) -> Result<*mut UObject> {
    if path.is_empty() || path.len() > crate::MAX_OBJECT_PATH_BYTES || path.contains('\0') {
        return Err(Error(
            "expected a nonempty full object path without NUL".into(),
        ));
    }
    let class = construct::require_class(class_name)?;
    find_object(Some(class), path).ok_or_else(|| Error(format!("loaded object not found: {path}")))
}

pub(crate) fn static_mesh(path: &str) -> Result<*mut UObject> {
    loaded_object(path, "Engine.StaticMesh")
}

pub(crate) fn skeletal_mesh(path: &str) -> Result<*mut UObject> {
    loaded_object(path, "Engine.SkeletalMesh")
}
