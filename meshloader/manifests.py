import tomllib
from collections.abc import Mapping
from dataclasses import dataclass, field
from pathlib import Path
from typing import cast

from ._revisions import source_revisions
from .defaults import DEFAULT_MATERIAL, DEFAULT_SCALE
from .models import (
    BufferRevisions,
    MaterialMapping,
    MeshKind,
    SourceRevision,
)
from .validate import kind as _kind
from .validate import material_mapping, object_path
from .validate import scale as _scale
from .validate import target as _target
from .validate import umodel as _umodel

_SOURCE_SUFFIXES = (".gltf", ".glb")
_KEYS = {
    "source",
    "name",
    "node",
    "scene",
    "kind",
    "outer",
    "scale",
    "umodel",
    "material",
    "materials",
}


@dataclass(frozen=True)
class MeshManifest:
    """Describe a validated mesh definition and its source revision hints.

    Relative source paths are resolved against the manifest.
    Unique scene and nodes identify one mesh instance.
    ``material`` supplies the fallback for source slots
    absent from ``materials``. ``umodel`` defaults to PSK-workflow
    bone frames. False preserves authored glTF frames.
    """

    manifest_path: Path
    source: Path
    name: str
    outer: str
    node: str
    scene: str
    kind: MeshKind = "auto"
    scale: float = DEFAULT_SCALE
    umodel: bool = True
    material: str = DEFAULT_MATERIAL
    materials: MaterialMapping = field(default_factory=dict)
    # Track source size and modification time for reload detection.
    source_revision: SourceRevision = (0, 0)
    # Track external .gltf buffer paths, sizes and modification times.
    buffer_revisions: BufferRevisions = ()

    @property
    def object_path(self) -> str:
        """Return the qualified engine object path this manifest creates."""
        return f"{self.outer}.{self.name}"


def _text(data: Mapping[str, object], key: str, default: str | None = None) -> str:
    value = data.get(key, default)
    if not isinstance(value, str) or not value.strip():
        raise ValueError(f"{key} must be a nonempty string")
    return value


def load_manifest(file: str | Path) -> MeshManifest:
    """Parse a TOML file into a validated mesh definition.

    ``source``, ``name``, ``node`` and ``scene`` are required.
    Defaults are ``kind="auto"``, ``scale=100.0``, ``umodel=True`` and
    ``EngineMaterials.DefaultMaterial``. All geometry uses (X,Z,Y) conversion.
    ``umodel=True`` converts PSK-workflow Y-up export bone frames.
    ``False`` preserves authored glTF frames.
    The ``[materials]`` table maps source indices, with ``default``
    for faces without a material.

    Unknown keys or invalid values raise ``ValueError``. File access errors
    raise ``OSError``.
    """
    file = Path(file).resolve()
    with file.open("rb") as stream:
        data = cast(dict[str, object], tomllib.load(stream))

    if unknown := (data.keys() - _KEYS):
        raise ValueError(f"unknown manifest keys: {', '.join(sorted(unknown))}")

    source = Path(_text(data, "source"))
    if not source.is_absolute():
        source = file.parent / source
    source = Path(source).resolve()
    if not source.is_file() or source.suffix.lower() not in _SOURCE_SUFFIXES:
        raise ValueError(f"source must be an existing .gltf/.glb file: {source}")

    name = _text(data, "name")
    outer = _text(data, "outer") if "outer" in data else None
    name, outer = _target(name, outer)
    kind = _kind(_text(data, "kind", "auto"))
    scale = _scale(data.get("scale", DEFAULT_SCALE))
    material = object_path(_text(data, "material", DEFAULT_MATERIAL))

    raw = data.get("materials", {})
    if not isinstance(raw, dict):
        raise ValueError("materials must be a TOML table of source IDs to engine paths")
    materials: MaterialMapping = {}
    for key, value in cast(dict[object, object], raw).items():
        if not isinstance(key, str) or not isinstance(value, str):
            raise ValueError("material keys and paths must be strings")
        if key == "default":
            slot = None
        elif key.isascii() and key.isdecimal():
            slot = int(key)
        else:
            raise ValueError(f"material key {key!r} must be a nonnegative source index or default")
        if slot in materials:
            raise ValueError(f"duplicate normalized source material ID: {key}")
        materials[slot] = value
    material_mapping(materials)

    revision, buffers = source_revisions(source)
    return MeshManifest(
        manifest_path=file,
        source=source,
        name=name,
        outer=outer,
        node=_text(data, "node"),
        scene=_text(data, "scene"),
        kind=kind,
        scale=scale,
        umodel=_umodel(data.get("umodel", True)),
        material=material,
        materials=materials,
        source_revision=revision,
        buffer_revisions=buffers,
    )


def manifest_files(directory: str | Path) -> list[Path]:
    """Discover TOML manifests recursively in deterministic order.

    Missing or empty directories produce an empty list.
    """
    directory = Path(directory)
    if not directory.exists():
        return []
    if not directory.is_dir():
        raise ValueError(f"manifest directory is not a directory: {directory}")
    return sorted(
        (p for p in directory.rglob("*.toml", case_sensitive=False) if p.is_file()),
        key=lambda p: (str(p).casefold(), str(p)),
    )
