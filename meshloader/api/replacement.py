from pathlib import Path
from typing import TYPE_CHECKING

from meshloader import validate
from meshloader._meshloader import load_into_skeletal_mesh as native_load_into_skeletal_mesh
from meshloader._meshloader import load_into_static_mesh as native_load_into_static_mesh
from meshloader.constants import DEFAULT_MATERIAL, DEFAULT_SCALE
from meshloader.models import MaterialMapping

from .loading import mesh_from_native

if TYPE_CHECKING:
    from unrealsdk.unreal import UObject


def _load_into(  # noqa: PLR0913
    source: str | Path,
    path: str,
    *,
    skeletal: bool,
    material: str,
    materials: MaterialMapping | None,
    scale: float,
) -> UObject:
    source, path = str(source), validate.object_path(path)
    scale = validate.scale(scale)
    mapping = validate.material_mapping(materials)

    native = native_load_into_skeletal_mesh if skeletal else native_load_into_static_mesh
    return mesh_from_native(
        native(source, path, material=material, materials=mapping, scale=scale),
    )


def load_into_static_mesh(
    source: str | Path,
    path: str,
    *,
    material: str = DEFAULT_MATERIAL,
    materials: MaterialMapping | None = None,
    scale: float = DEFAULT_SCALE,
) -> UObject:
    """Replace an existing StaticMesh's geometry and return the same object.

    Sockets and simple collision are preserved.
    Instanced meshes and unsupported component states prevent replacement.
    Collision and baked lighting are not regenerated.
    """
    return _load_into(
        source,
        path,
        skeletal=False,
        material=material,
        materials=materials,
        scale=scale,
    )


def load_into_skeletal_mesh(
    source: str | Path,
    path: str,
    *,
    material: str = DEFAULT_MATERIAL,
    materials: MaterialMapping | None = None,
    scale: float = DEFAULT_SCALE,
) -> UObject:
    """Replace an existing SkeletalMesh's geometry and return the same object.

    The original skeleton, sockets, and animation caches are preserved.
    Bone names, order, parents and rest frames must match.
    """
    return _load_into(
        source,
        path,
        skeletal=True,
        material=material,
        materials=materials,
        scale=scale,
    )
