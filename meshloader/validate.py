from typing import cast

from .constants import MAX_SCALE
from .models import MaterialMapping, MeshKind


def target(name: str, outer: str | None) -> tuple[str, str]:
    """Resolve an explicit name and optional outer.

    Bare names default to ``Transient``. Qualified ``Package.Name`` values
    supply the outer, so a separate ``outer`` is not allowed.
    """
    if not isinstance(name, str) or not name:
        raise ValueError("name must be a nonempty mesh identifier or qualified Package.Name")
    if "." in name:
        if outer is not None:
            raise ValueError("use a qualified name OR outer, not both")
        outer, name = name.rsplit(".", 1)
    outer = "Transient" if outer is None else outer
    return name, outer


def object_path(value: object) -> str:
    """Validate an existing object's full path."""
    if (
        not isinstance(value, str)
        or not value
        or "." not in value
        or any(not part or part != part.strip() for part in value.split("."))
    ):
        raise ValueError("path must be a nonempty full object path")
    return value


def kind(value: object) -> MeshKind:
    """Validate and lowercase the mesh kind (auto, static or skeletal)."""
    if not isinstance(value, str) or value.lower() not in (
        "auto",
        "static",
        "skeletal",
    ):
        raise ValueError("kind must be auto, static or skeletal")
    return cast(MeshKind, value.lower())


def scale(value: object) -> float:
    """Validate a positive finite scale that remains nonzero in native f32."""
    if not isinstance(value, (int, float)) or not 0 < value <= MAX_SCALE:
        raise ValueError("scale must be positive and representable as f32")
    return float(value)


def material_mapping(materials: MaterialMapping | None) -> MaterialMapping | None:
    """Validate and copy source-material assignments for the native binding.

    Keys are non-negative source material indices or ``None`` for unassigned
    faces. Values are nonempty engine material paths. ``None`` input remains
    ``None``.
    """
    if materials is None:
        return None
    if not isinstance(materials, dict):
        raise TypeError("materials must be a dict of glTF material indices (or None) to Material paths")
    mapping: MaterialMapping = {}
    for key, value in materials.items():
        if key is not None and (type(key) is not int or key < 0):
            raise TypeError("material keys must be nonnegative integer glTF indices or None")
        if not isinstance(value, str) or not value:
            raise ValueError("material paths must be nonempty bounded strings")
        mapping[key] = value
    return mapping
