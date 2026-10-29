import json
import tomllib
from collections.abc import Mapping
from dataclasses import dataclass, field
from pathlib import Path
from typing import cast

from .constants import DEFAULT_MATERIAL, DEFAULT_SCALE, MAX_GLTF_SOURCE_BYTES, SOURCE_SUFFIXES
from .models import (
    BufferRevisions,
    JsonValue,
    MaterialMapping,
    MeshKind,
    SourceRevision,
)
from .validate import kind as _kind
from .validate import material_mapping
from .validate import scale as _scale
from .validate import target as _target

CREATE_MESHES = Path(__file__).parent / "create_meshes"
_KEYS = {
    "source",
    "name",
    "kind",
    "outer",
    "scale",
    "material",
    "materials",
}


@dataclass(frozen=True)
class MeshManifest:
    """Describe a validated mesh definition and its source revision hints.

    Relative source paths are resolved against the manifest. ``material`` is
    the fallback, while ``materials`` overrides source glTF slots.
    """

    manifest_path: Path
    source: Path
    name: str
    outer: str
    kind: MeshKind = "auto"
    scale: float = DEFAULT_SCALE
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
    return value.strip()


def _buffer_revisions(source: Path) -> BufferRevisions:
    """Return paths, sizes and modification times of external .gltf buffers."""
    if source.suffix.lower() != ".gltf":
        return ()
    if source.stat().st_size > MAX_GLTF_SOURCE_BYTES:
        raise ValueError("glTF source exceeds the signed 32-bit buffer size")
    data = source.read_bytes()
    if len(data) > MAX_GLTF_SOURCE_BYTES:
        raise ValueError("glTF source exceeds the signed 32-bit buffer size")
    document = cast(JsonValue, json.loads(data))
    if not isinstance(document, dict):
        raise ValueError("glTF document must be an object")
    paths = set()
    buffers = document.get("buffers", [])
    if not isinstance(buffers, list):
        raise ValueError("glTF buffers must be an array")
    for buffer in buffers:
        if not isinstance(buffer, dict):
            raise ValueError("glTF buffer must be an object")
        uri = buffer.get("uri")
        if not isinstance(uri, str) or not uri:
            raise ValueError("glTF buffer URI must be a plain relative path")
        path = Path(uri)
        if path.is_absolute() or ".." in path.parts or any(c in uri for c in ":\\%?#\0"):
            raise ValueError("glTF buffer URI must be a plain relative path")
        path = (source.parent / path).resolve(strict=True)
        if not path.is_relative_to(source.parent):
            raise ValueError("glTF buffer resolves outside its asset folder")
        paths.add(path)
    revisions = []
    for path in sorted(paths):
        stat = path.stat()
        revisions.append((str(path), stat.st_size, stat.st_mtime_ns))
    return tuple(revisions)


def load_manifest(file: str | Path) -> MeshManifest:
    """Parse a TOML file into a validated mesh definition.

    ``source`` and ``name`` are required. Defaults are ``kind="auto"``,
    ``scale=100.0`` and ``EngineMaterials.DefaultMaterial``. All sources use
    the fixed (X,Z,Y) axis conversion. The ``[materials]`` table maps source
    indices, with ``default`` for faces without a material.

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
    if not source.is_file() or source.suffix.lower() not in SOURCE_SUFFIXES:
        raise ValueError(f"source must be an existing .gltf/.glb file: {source}")

    name = _text(data, "name")
    outer = _text(data, "outer") if "outer" in data else None
    name, outer = _target(name, outer)
    kind = _kind(_text(data, "kind", "auto"))
    scale = _scale(data.get("scale", DEFAULT_SCALE))
    material = _text(data, "material", DEFAULT_MATERIAL)

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

    stat = source.stat()
    buffers = _buffer_revisions(source)
    return MeshManifest(
        manifest_path=file,
        source=source,
        name=name,
        outer=outer,
        kind=kind,
        scale=scale,
        material=material,
        materials=materials,
        source_revision=(stat.st_size, stat.st_mtime_ns),
        buffer_revisions=buffers,
    )


def manifest_files(directory: str | Path = CREATE_MESHES) -> list[Path]:
    """Discover TOML manifests recursively in deterministic order.

    Missing or empty directories produce an empty list.
    """
    directory = Path(directory)
    if not directory.exists():
        return []
    if not directory.is_dir():
        raise ValueError(f"manifest directory is not a directory: {directory}")
    return sorted(
        (p for p in directory.rglob("*") if p.is_file() and p.suffix.lower() == ".toml"),
        key=lambda p: (str(p).casefold(), str(p)),
    )
