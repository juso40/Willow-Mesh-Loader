from pathlib import Path
from typing import cast

from meshloader import validate
from meshloader._meshloader import inspect_mesh
from meshloader.constants import DEFAULT_SCALE
from meshloader.models import MeshInspection, MeshKind, SkeletalInspection, StaticInspection


def inspect(
    source: str | Path,
    *,
    kind: MeshKind = "auto",
    scale: float = DEFAULT_SCALE,
) -> MeshInspection:
    """Inspect source geometry, material slots and prepared payload metadata.

    Auto detection selects skeletal import for sources with a skin.
    Every import maps glTF (X,Y,Z) to engine (X,Z,Y), multiplied by scale.
    """
    return cast(
        MeshInspection,
        inspect_mesh(str(source), validate.kind(kind), validate.scale(scale)),
    )


def inspect_gltf(
    source: str | Path,
    *,
    scale: float = DEFAULT_SCALE,
) -> StaticInspection:
    """Inspect static geometry, material slots and bounds."""
    return cast(StaticInspection, inspect(source, kind="static", scale=scale))


def inspect_skeletal_gltf(
    source: str | Path,
    *,
    scale: float = DEFAULT_SCALE,
) -> SkeletalInspection:
    """Inspect one skin, its reference skeleton and prepared payload."""
    return cast(SkeletalInspection, inspect(source, kind="skeletal", scale=scale))
