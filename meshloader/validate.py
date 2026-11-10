"""Validate Python-facing names and options before native preparation."""

from collections.abc import Sequence
from math import isfinite
from re import fullmatch
from struct import pack, unpack
from typing import Protocol, cast

from .models import MaterialMapping, MeshKind

_MAX_NAME_UTF16_UNITS = 1023


def target(name: str, outer: str | None) -> tuple[str, str]:
    """Resolve a validated object name and an existing package path.

    Bare names default to Transient. A qualified Package.Name supplies the
    outer, so a separate outer is not allowed.
    """
    if not isinstance(name, str):
        raise TypeError("name must be a string")
    if "." in name:
        if outer is not None:
            raise ValueError("use a qualified name OR outer, not both")
        outer, name = name.rsplit(".", 1)
    name = object_name(name)
    outer = "Transient" if outer is None else outer
    if not _path_components(outer):
        raise ValueError("outer must be a nonempty package path")
    return name, outer


def object_name(name: str) -> str:
    """Require a bounded ASCII identifier that is not the reserved None name."""
    if not isinstance(name, str):
        raise TypeError("name must be a string")
    if (
        fullmatch(r"[A-Za-z][A-Za-z0-9_]*", name) is None
        or len(name) > _MAX_NAME_UTF16_UNITS
        or name.casefold() == "none"
    ):
        raise ValueError(
            "name must start with a letter, contain only ASCII letters/digits/underscores, "
            "fit 1023 characters, and not be None",
        )
    return name


class _NamedSource(Protocol):
    @property
    def id(self) -> int: ...

    @property
    def name(self) -> str | None: ...


def source_index(entries: Sequence[_NamedSource], name: str, label: str) -> int:
    """Resolve one exact authored name to its internal source index.

    Empty, missing or ambiguous names raise ValueError.
    Names are case-sensitive and are not stripped or normalized.
    """
    if not name:
        raise ValueError(f"{label} name must be nonempty")
    matches = [entry for entry in entries if entry.name == name]
    if len(matches) != 1:
        raise ValueError(f"{label} name {name!r} must match exactly one source entry, found {len(matches)}")
    return matches[0].id


def _path_components(value: object) -> bool:
    return isinstance(value, str) and bool(value) and all(part and part == part.strip() for part in value.split("."))


def object_path(value: object) -> str:
    """Require an existing object's nonempty full path."""
    if not _path_components(value) or "." not in cast(str, value):
        raise ValueError("path must be a nonempty full object path")
    return cast(str, value)


def kind(value: object) -> MeshKind:
    """Validate and lowercase the mesh kind: auto, static or skeletal."""
    if not isinstance(value, str) or value.lower() not in ("auto", "static", "skeletal"):
        raise ValueError("kind must be auto, static or skeletal")
    return cast(MeshKind, value.lower())


def _positive_f32(value: object, label: str) -> float:
    message = f"{label} must be finite, positive and representable as nonzero f32"
    if not isinstance(value, (int, float)):
        raise ValueError(message)
    try:
        result = float(value)
        if not isfinite(result) or result <= 0 or unpack("<f", pack("<f", result))[0] == 0:
            raise ValueError(message)
    except OverflowError as error:
        raise ValueError(message) from error
    return result


def scale(value: object) -> float:
    """Require a finite positive scale that remains nonzero in native f32."""
    return _positive_f32(value, "scale")


def sample_rate(value: object) -> float:
    """Require a finite positive sample rate that remains nonzero in native f32."""
    return _positive_f32(value, "sample_rate")


def umodel(value: object) -> bool:
    """Validate the explicit PSK-workflow flag without coercing other types."""
    if not isinstance(value, bool):
        raise ValueError("umodel must be a boolean")
    return value


def material_mapping(materials: MaterialMapping | None) -> MaterialMapping | None:
    """Validate and copy source-material assignments for the native binding.

    Keys are non-negative glTF material indices or None for unassigned faces.
    Values are nonempty engine material paths. None input remains None.
    """
    if materials is None:
        return None
    if not isinstance(materials, dict):
        raise TypeError("materials must be a dict of glTF material indices (or None) to Material paths")
    mapping: MaterialMapping = {}
    for key, value in materials.items():
        if key is not None and (type(key) is not int or key < 0):
            raise TypeError("material keys must be nonnegative integer glTF indices or None")
        if not isinstance(value, str) or not value or "\0" in value:
            raise ValueError("material paths must be nonempty strings without NUL")
        mapping[key] = value
    return mapping
