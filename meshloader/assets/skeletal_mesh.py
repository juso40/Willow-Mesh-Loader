from pathlib import Path
from typing import TYPE_CHECKING, cast

from meshloader._meshloader import get_reference_skeleton as native_get_reference_skeleton
from meshloader.defaults import DEFAULT_MATERIAL, DEFAULT_SCALE
from meshloader.models import MaterialMapping, ReferenceBone, SkeletalInspection
from meshloader.sources import Source

from . import meshes

if TYPE_CHECKING:
    from unrealsdk.unreal import UObject


def inspect_skeletal_mesh(
    source: str | Path | Source,
    *,
    node: str,
    scene: str,
    scale: float = DEFAULT_SCALE,
    umodel: bool = True,
) -> SkeletalInspection:
    """Inspect selected skin geometry, reference bones and prepared payload metadata."""
    info = meshes.inspect(source, node=node, scene=scene, kind="skeletal", scale=scale, umodel=umodel)
    return cast(SkeletalInspection, info)


def load_skeletal_mesh(  # noqa: PLR0913
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
    """Create skinned geometry selected by exact mesh-node and scene names."""
    return meshes.load(
        source,
        name,
        kind="skeletal",
        outer=outer,
        material=material,
        materials=materials,
        scale=scale,
        umodel=umodel,
        node=node,
        scene=scene,
    )


def load_into_skeletal_mesh(  # noqa: PLR0913
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
    """Replace an existing SkeletalMesh's geometry and retain its identity.

    The skeleton, sockets and animation caches are preserved. Bone names, order,
    parents and rest frames must match. Node and scene names require exact,
    unique matches.
    """
    return meshes.replace(
        source,
        path,
        kind="skeletal",
        material=material,
        materials=materials,
        scale=scale,
        umodel=umodel,
        node=node,
        scene=scene,
    )


def get_reference_skeleton(mesh: UObject) -> list[ReferenceBone]:
    """Read a loaded SkeletalMesh's reference bones in native order.

    Records contain names, parent indices and local reference frames. Root
    parents are None.
    """
    return native_get_reference_skeleton(mesh._path_name())
