from typing import TYPE_CHECKING

import unrealsdk
from mods_base import ObjectFlags
from unrealsdk import logging

from meshloader import validate

if TYPE_CHECKING:
    from unrealsdk.unreal import UObject

    from meshloader._meshloader import AnimationOutcome, GestaltAppendOutcome, MeshOutcome


def path(value: str | UObject) -> str:
    """Resolve an asset reference to a full object path."""
    if isinstance(value, str):
        return validate.object_path(value)
    return validate.object_path(value._path_name())


def require_unique(value: str) -> None:
    """Reject an already occupied object path."""
    try:
        existing = unrealsdk.find_object("Object", value)
    except ValueError:
        return
    if existing is not None:
        raise ValueError(f"Object already exists: {value}")


def package(value: str) -> UObject:
    """Resolve a loaded package."""
    try:
        result = unrealsdk.find_object("Package", value)
    except ValueError as err:
        raise ValueError(f"Package is not loaded: {value}") from err
    if result is None:
        raise ValueError(f"Package is not loaded: {value}")
    return result


def construct(class_name: str, outer: UObject, name: str) -> UObject:
    """Construct a new empty UObject."""
    validate.object_name(name)
    require_unique(f"{outer._path_name()}.{name}")
    result = unrealsdk.construct_object(
        class_name,
        outer,
        name,
        flags=ObjectFlags.KEEP_ALIVE | ObjectFlags.ALLOW_CROSS_PACKAGE_REFERENCES,
    )
    if result is None:
        raise RuntimeError(f"{class_name} construction failed: {name}")
    return result


def clone(template: UObject, outer: UObject, *, name: str | None = None) -> UObject:
    """Clone a loaded UObject under a new outer with the shared keep-alive flags.

    An explicit name uses the engine object-name restrictions. An inherited native
    name is preserved without revalidating the original asset's name.
    """
    if name is not None:
        validate.object_name(name)
        require_unique(f"{outer._path_name()}.{name}")
    result = unrealsdk.construct_object(
        template.Class,
        outer,
        template.Name if name is None else name,
        flags=ObjectFlags.KEEP_ALIVE | ObjectFlags.ALLOW_CROSS_PACKAGE_REFERENCES,
        template_obj=template,
    )
    if result is None:
        raise RuntimeError("UObject cloning failed")
    return result


def copy_sockets(template_mesh: UObject, mesh: UObject) -> None:
    """Clone socket UObjects under a mesh instead of sharing mutable originals."""
    mesh.Sockets = [clone(socket, mesh) for socket in template_mesh.Sockets]


def finish(outcome: MeshOutcome | AnimationOutcome | GestaltAppendOutcome, class_name: str = "Object") -> UObject:
    """Log import warnings and resolve a populated or replaced asset."""
    for warning in outcome.warnings:
        logging.warning(f"{outcome.path}: {warning}")
    return unrealsdk.find_object(class_name, outcome.path)
