from pathlib import Path
from typing import TYPE_CHECKING, cast

from meshloader.defaults import DEFAULT_MATERIAL, DEFAULT_SCALE
from meshloader.models import MaterialMapping, StaticInspection
from meshloader.sources import Source

from . import meshes

if TYPE_CHECKING:
    from unrealsdk.unreal import UObject


def inspect_static_mesh(
    source: str | Path | Source,
    *,
    node: str,
    scene: str,
    scale: float = DEFAULT_SCALE,
    umodel: bool = True,
) -> StaticInspection:
    """Inspect selected static geometry, source material slots and bounds."""
    return cast(
        StaticInspection,
        meshes.inspect(source, node=node, scene=scene, kind="static", scale=scale, umodel=umodel),
    )


def load_static_mesh(  # noqa: PLR0913
    source: str | Path | Source,
    name: str,
    *,
    outer: str | None = None,
    material: str = DEFAULT_MATERIAL,
    scale: float = DEFAULT_SCALE,
    umodel: bool = True,
    materials: MaterialMapping | None = None,
    node: str,
    scene: str,
) -> UObject:
    """Create static geometry selected by exact mesh-node and scene names."""
    return meshes.load(
        source,
        name,
        kind="static",
        outer=outer,
        material=material,
        materials=materials,
        scale=scale,
        umodel=umodel,
        node=node,
        scene=scene,
    )


def load_into_static_mesh(  # noqa: PLR0913
    source: str | Path | Source,
    path: str,
    *,
    material: str = DEFAULT_MATERIAL,
    materials: MaterialMapping | None = None,
    scale: float = DEFAULT_SCALE,
    umodel: bool = True,
    node: str,
    scene: str,
) -> UObject:
    """Replace an existing StaticMesh's geometry and retain its identity.

    Sockets and simple collision are preserved. Unsupported component states and
    triangle collision prevent replacement. Collision and baked lighting are not
    regenerated. Node and scene names require exact, unique matches.
    """
    return meshes.replace(
        source,
        path,
        kind="static",
        material=material,
        materials=materials,
        scale=scale,
        umodel=umodel,
        node=node,
        scene=scene,
    )
