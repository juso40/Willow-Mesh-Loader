from pathlib import Path
from typing import TYPE_CHECKING

from meshloader import validate
from meshloader.defaults import DEFAULT_MATERIAL, DEFAULT_SCALE
from meshloader.models import MaterialMapping, MeshInspection, MeshKind
from meshloader.sources import Source, open_source

if TYPE_CHECKING:
    from unrealsdk.unreal import UObject


def inspect(  # noqa: PLR0913
    source: str | Path | Source,
    *,
    node: str,
    scene: str,
    kind: MeshKind = "auto",
    scale: float = DEFAULT_SCALE,
    umodel: bool = True,
) -> MeshInspection:
    """Inspect a named mesh node's geometry, slots and prepared payload.

    Names require exact, unique catalog matches. Auto detection uses the node's
    skin reference. Coordinates map glTF (X,Y,Z) to engine (X,Z,Y), multiplied by
    scale. ``umodel`` selects the bone-frame workflow, not geometry conversion.
    """
    return open_source(source).prepare_mesh(node=node, scene=scene, kind=kind, scale=scale, umodel=umodel).metadata


def load(  # noqa: PLR0913
    source: str | Path | Source,
    name: str,
    *,
    kind: MeshKind = "auto",
    outer: str | None = None,
    material: str = DEFAULT_MATERIAL,
    materials: MaterialMapping | None = None,
    scale: float = DEFAULT_SCALE,
    umodel: bool = True,
    node: str,
    scene: str,
) -> UObject:
    """Prepare geometry, construct a fresh mesh and populate its native storage.

    Existing objects are not replaced. Bare names default to Transient. Qualified
    names or ``outer`` select an existing package. ``materials`` maps original
    glTF indices, or None for unassigned faces, to loaded material paths.
    ``material`` supplies unmapped slots. ``umodel=True`` selects PSK-workflow
    bone frames. False preserves authored glTF frames.
    """
    name, outer = validate.target(name, outer)
    prepared = open_source(source).prepare_mesh(
        node=node,
        scene=scene,
        kind=kind,
        material=material,
        materials=materials,
        scale=scale,
        umodel=umodel,
    )
    return prepared.create(name, outer=outer)


def reload(  # noqa: PLR0913
    source: str | Path | Source,
    name: str,
    *,
    kind: MeshKind = "auto",
    outer: str | None = None,
    material: str = DEFAULT_MATERIAL,
    materials: MaterialMapping | None = None,
    scale: float = DEFAULT_SCALE,
    umodel: bool = True,
    node: str,
    scene: str,
) -> UObject:
    """Replace library-created geometry and retain the same mesh identity.

    Skeletal reload requires unchanged bone names, order, parents and rest frames.
    Sockets and animation caches remain unchanged. Original game meshes use
    ``load_into_*`` instead. Node and scene names require exact, unique matches.
    """
    name, outer = validate.target(name, outer)
    prepared = open_source(source).prepare_mesh(
        node=node,
        scene=scene,
        kind=kind,
        material=material,
        materials=materials,
        scale=scale,
        umodel=umodel,
    )
    return prepared.reload(f"{outer}.{name}")


def replace(  # noqa: PLR0913
    source: str | Path | Source,
    path: str,
    *,
    kind: MeshKind,
    material: str,
    materials: MaterialMapping | None,
    scale: float,
    umodel: bool,
    node: str,
    scene: str,
) -> UObject:
    """Prepare geometry and replace an existing mesh under its native restrictions."""
    path = validate.object_path(path)
    prepared = open_source(source).prepare_mesh(
        node=node,
        scene=scene,
        kind=kind,
        material=material,
        materials=materials,
        scale=scale,
        umodel=umodel,
    )
    return prepared.load_into(path)
