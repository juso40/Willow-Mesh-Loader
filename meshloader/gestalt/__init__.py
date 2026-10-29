from dataclasses import dataclass
from pathlib import Path
from typing import TYPE_CHECKING

import unrealsdk

from meshloader._meshloader import load_gestalt_mesh
from meshloader.api.inspection import inspect_skeletal_gltf
from meshloader.api.loading import mesh_from_native
from meshloader.constants import DEFAULT_MATERIAL
from meshloader.models import SkeletalInspection
from meshloader.validate import target

from .definition import publish_definition, template_parts
from .metadata import GestaltMetadata, load_metadata

if TYPE_CHECKING:
    from unrealsdk.unreal import UObject

__all__ = ["GestaltAsset", "inspect_gestalt", "load_gestalt"]


@dataclass(frozen=True)
class GestaltAsset:
    """Hold a mesh and its matching Gestalt definition."""

    mesh: UObject
    definition: UObject


def inspect_gestalt(source: str | Path) -> SkeletalInspection:
    """Inspect final ranges, bounds and skinning without creating engine objects."""
    metadata = load_metadata(source)
    info = inspect_skeletal_gltf(metadata.source, scale=metadata.scale)
    parts = info["gestalt_parts"]
    if {part["name"].casefold() for part in parts} != {name.casefold() for name in metadata.fragment_names}:
        raise ValueError("GLB fragment tags do not match the Gestalt sidecar")
    cursor = 0
    for part in parts:
        if part["first_index"] != cursor or part["triangles"] <= 0:
            raise ValueError("Gestalt tags must cover every triangle exactly once")
        cursor += part["triangles"] * 3
    if cursor != info["triangles"] * 3 or info["tex_coords"] != metadata.uv_channels:
        raise ValueError("Gestalt triangle coverage or UV count does not match the export metadata")
    for section in info["sections"]:
        slot = section["source_material"]
        if slot is None or slot >= len(metadata.material_slots):
            raise ValueError("GLB materials do not match the Gestalt template slot map")
    for part in parts:
        source_slot = info["sections"][part["material_index"]]["source_material"]
        if source_slot is not None:
            part["material_index"] = metadata.material_slots[source_slot]
    return info


def _require_unique(path: str) -> None:
    try:
        existing = unrealsdk.find_object("Object", path)
    except ValueError:
        return
    if existing is not None:
        raise ValueError(f"Object already exists: {path}. Gestalt loading does not replace live assets")


def _material_paths(mesh: UObject, metadata: GestaltMetadata) -> list[str]:
    materials = list(mesh.Materials)
    if not materials or any(slot >= len(materials) for slot in metadata.material_slots):
        raise ValueError("Exported material slots exceed the template mesh's materials")
    return [DEFAULT_MATERIAL if material is None else material._path_name() for material in materials]


def load_gestalt(
    source: str | Path,
    name: str,
    *,
    outer: str | None = None,
    mesh_name: str | None = None,
    template: str | None = None,
) -> GestaltAsset:
    """Load an add-on export into a fresh mesh and Gestalt definition.

    The sidecar selects an already loaded template unless overridden.
    Its skeleton, fragment identities, material slots and accessory mappings
    are preserved, with cloned sockets and recomputed ranges and bounds.
    The returned definition belongs in ``WeaponTypeDefinition.GestaltMesh``
    before new weapon assembly. Existing weapons are unchanged.
    """
    metadata = load_metadata(source)
    name, outer = target(name, outer)
    mesh_name, _ = target(mesh_name if mesh_name is not None else f"{name}_Mesh", outer)
    if name.casefold() == mesh_name.casefold():
        raise ValueError("Mesh and definition names must be distinct")
    _require_unique(f"{outer}.{name}")
    _require_unique(f"{outer}.{mesh_name}")
    original = unrealsdk.find_object("GestaltSkeletalMeshDefinition", template or metadata.template_definition)
    if original is None:
        raise ValueError("The original Gestalt definition must already be loaded")
    prototypes = template_parts(original)
    if set(prototypes) != {fragment.casefold() for fragment in metadata.fragment_names}:
        raise ValueError("The loaded template and source must contain exactly the same fragment identities")
    template_mesh = original.GestaltSkeletalMesh
    if template_mesh._path_name().casefold() != metadata.template_mesh.casefold():
        raise ValueError("The loaded template mesh does not match the source dump")
    outcome = load_gestalt_mesh(
        str(metadata.source),
        mesh_name,
        template_mesh._path_name(),
        metadata.fragment_names,
        _material_paths(template_mesh, metadata),
        metadata.material_slots,
        metadata.uv_channels,
        outer=outer,
        scale=metadata.scale,
    )
    mesh = mesh_from_native(outcome.mesh)
    definition = publish_definition(original, mesh, name, outcome.parts)
    return GestaltAsset(mesh, definition)
