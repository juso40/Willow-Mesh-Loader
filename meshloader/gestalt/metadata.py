import hashlib
import json
from dataclasses import dataclass
from pathlib import Path

from meshloader.validate import object_path, scale


@dataclass(frozen=True)
class GestaltMetadata:
    """Describe one hash-bound LOD0 Gestalt source and its template mapping."""

    source: Path
    template_definition: str
    template_mesh: str
    material_slots: tuple[int, ...]
    fragment_names: tuple[str, ...]
    uv_channels: int
    scale: float


def load_metadata(source: str | Path) -> GestaltMetadata:
    """Validate the adjacent sidecar and verify the exported GLB digest."""
    source = Path(source).expanduser().resolve(strict=True)
    if source.suffix.lower() != ".glb":
        raise ValueError("Gestalt loading requires the add-on's GLB and .gestalt.json pair")
    data = json.loads(source.with_suffix(".gestalt.json").read_text(encoding="utf-8"))
    if (
        not isinstance(data, dict)
        or data.get("format") != "meshloader.gestalt"
        or type(data.get("version")) is not int
        or data["version"] != 1
    ):
        raise ValueError("Unsupported Gestalt sidecar format/version")
    if type(data.get("lod")) is not int or data["lod"] != 0:
        raise ValueError("Gestalt loading currently supports LOD0 only")
    with source.open("rb") as stream:
        digest = hashlib.file_digest(stream, "sha256").hexdigest()
    if digest != data.get("source_sha256"):
        raise ValueError("GLB and Gestalt sidecar do not match. Re-export both files")
    slots = data.get("material_slots")
    if (
        not isinstance(slots, list)
        or not slots
        or any(type(slot) is not int or not 0 <= slot <= 65535 for slot in slots)
        or len(slots) != len(set(slots))
    ):
        raise ValueError("material_slots must contain distinct native u16 material ordinals")
    names = data.get("fragment_names")
    if (
        not isinstance(names, list)
        or not names
        or any(
            not isinstance(name, str)
            or not name
            or name.casefold() == "none"
            or "\0" in name
            or len(name.encode("utf-16-le")) // 2 > 1023
            for name in names
        )
    ):
        raise ValueError("fragment_names must contain nonempty native FName identities")
    if len(names) != len({name.casefold() for name in names}):
        raise ValueError("Fragment identities must be unique, ignoring case")
    channels = data.get("uv_channels")
    if type(channels) is not int or not 1 <= channels <= 4:
        raise ValueError("Native GPU skinning supports one through four UV channels")
    return GestaltMetadata(
        source,
        object_path(data.get("template_definition")),
        object_path(data.get("template_mesh")),
        tuple(slots),
        tuple(names),
        channels,
        scale(data.get("scale")),
    )
