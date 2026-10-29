from pathlib import Path
from typing import TYPE_CHECKING

import unrealsdk
from unrealsdk import logging

from meshloader import validate
from meshloader._meshloader import MeshOutcome, load_mesh, reload_mesh
from meshloader.constants import DEFAULT_MATERIAL, DEFAULT_SCALE
from meshloader.models import MaterialMapping, MeshKind

if TYPE_CHECKING:
    from unrealsdk.unreal import UObject


def mesh_from_native(outcome: MeshOutcome) -> UObject:
    """Resolve a native mesh identity and log its import warnings once."""
    for warning in outcome.warnings:
        logging.warning(f"{outcome.path}: {warning}")
    return unrealsdk.find_object("Object", outcome.path)


def load(  # noqa: PLR0913
    source: str | Path,
    name: str,
    *,
    kind: MeshKind = "auto",
    outer: str | None = None,
    material: str = DEFAULT_MATERIAL,
    materials: MaterialMapping | None = None,
    scale: float = DEFAULT_SCALE,
) -> UObject:
    """Load a new static or skeletal mesh and return its UObject.

    Bare names default to ``Transient``. Qualified names or ``outer`` select
    an existing package. Existing objects are not replaced.
    ``materials`` maps source glTF indices, or ``None`` for unassigned faces,
    to loaded material paths. ``material`` supplies unmapped slots.
    """
    name, outer = validate.target(name, outer)
    outcome = load_mesh(
        str(source),
        name,
        validate.kind(kind),
        outer,
        material,
        validate.scale(scale),
        validate.material_mapping(materials),
    )
    return mesh_from_native(outcome)


def reload(  # noqa: PLR0913
    source: str | Path,
    name: str,
    *,
    kind: MeshKind = "auto",
    outer: str | None = None,
    material: str = DEFAULT_MATERIAL,
    materials: MaterialMapping | None = None,
    scale: float = DEFAULT_SCALE,
) -> UObject:
    """Replace library-created geometry and return the same mesh.

    Skeletal reload requires matching bone names, order, parents and rest frames.
    Sockets and animation caches remain unchanged.
    Original game meshes use ``load_into_*`` instead.
    """
    name, outer = validate.target(name, outer)
    outcome = reload_mesh(
        str(source),
        f"{outer}.{name}",
        validate.kind(kind),
        material,
        validate.scale(scale),
        validate.material_mapping(materials),
    )
    return mesh_from_native(outcome)


def load_static_mesh(  # noqa: PLR0913
    source: str | Path,
    name: str,
    *,
    outer: str | None = None,
    material: str = DEFAULT_MATERIAL,
    scale: float = DEFAULT_SCALE,
    materials: MaterialMapping | None = None,
) -> UObject:
    """Load static geometry and return its StaticMesh."""
    return load(
        source,
        name,
        kind="static",
        outer=outer,
        material=material,
        scale=scale,
        materials=materials,
    )


def load_skeletal_mesh(  # noqa: PLR0913
    source: str | Path,
    name: str,
    *,
    outer: str | None = None,
    material: str = DEFAULT_MATERIAL,
    scale: float = DEFAULT_SCALE,
    materials: MaterialMapping | None = None,
) -> UObject:
    """Load skinned geometry and return its SkeletalMesh."""
    return load(
        source,
        name,
        kind="skeletal",
        outer=outer,
        material=material,
        scale=scale,
        materials=materials,
    )
