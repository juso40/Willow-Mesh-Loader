from copy import deepcopy
from dataclasses import dataclass
from typing import TYPE_CHECKING

import unrealsdk

from meshloader import objects

if TYPE_CHECKING:
    from unrealsdk.unreal import UObject, WrappedStruct

    from meshloader._meshloader import FragmentRange


@dataclass(frozen=True)
class GestaltAsset:
    """Hold a mesh and its matching Gestalt definition."""

    mesh: UObject
    definition: UObject


def template_parts(template: UObject) -> dict[str, WrappedStruct]:
    """Return LOD0 parts indexed by case-insensitive fragment identity."""
    if template.bStaticMeshRigidBodyMesh or template.GestaltSkeletalMesh is None:
        raise ValueError("The target must be a non-rigid-body Gestalt skeletal mesh definition")
    if not template.GestaltInfos or not template.GestaltInfos[0].Parts:
        raise ValueError("Gestalt operations require a nonempty LOD0 part table")
    return {str(part.SkeletalMeshFragmentName).casefold(): part for part in template.GestaltInfos[0].Parts}


def check_names(parts: list[FragmentRange]) -> set[str]:
    """Validate fragment names without changing authored native identities."""
    names = {}
    for part in parts:
        name = part.name
        if not name or name.casefold() == "none" or "\0" in name or len(name.encode("utf-16-le")) // 2 > 1023:
            raise ValueError("Gestalt fragments require nonempty bounded FName identities")
        key = name.casefold()
        if key in names and names[key] != name:
            raise ValueError("Distinct fragment identities must not differ only by case")
        names[key] = name
    if not names:
        raise ValueError("A Gestalt mesh requires at least one fragment")
    return set(names)


def definition_values(
    template: UObject | None,
    parts: list[FragmentRange],
) -> tuple[WrappedStruct, list[WrappedStruct]]:
    """Build final range and bounds tables, including previously unknown fragments."""
    check_names(parts)
    prototypes = {} if template is None else template_parts(template)
    info = unrealsdk.make_struct("GestaltInfo")
    rows = []
    bounds = {}
    for part in parts:
        key = part.name.casefold()
        prototype = prototypes.get(key)
        row = unrealsdk.make_struct("GestaltPart") if prototype is None else deepcopy(prototype)
        row.SkeletalMeshFragment = None
        row.SkeletalMeshFragmentName = part.name
        row.MaterialIndex = part.material_slot
        row.FirstIndex = part.first_index
        row.NumPrimitives = part.triangle_count
        rows.append(row)
        if key not in bounds:
            item = unrealsdk.make_struct("GestaltPartBoundsEntry")
            item.SkeletalMeshFragmentName = part.name
            values = part.bounds
            for axis, origin, extent in zip("XYZ", values.origin, values.box_extent, strict=True):
                setattr(item.ReferencePoseBounds.Origin, axis, origin)
                setattr(item.ReferencePoseBounds.BoxExtent, axis, extent)
            item.ReferencePoseBounds.SphereRadius = values.sphere_radius
            bounds[key] = item
    info.Parts = rows
    return info, list(bounds.values())


def require_exclusive_mesh(definition: UObject) -> None:
    """Reject full replacement when another definition would retain invalid ranges."""
    template_parts(definition)
    if len(definition.GestaltInfos) != 1:
        raise ValueError("In-place Gestalt replacement supports a single LOD0 table")
    mesh = definition.GestaltSkeletalMesh
    if any(
        other != definition and other.GestaltSkeletalMesh == mesh
        for other in unrealsdk.find_all("GestaltSkeletalMeshDefinition")
    ):
        raise ValueError("Gestalt replacement refuses a master shared by another definition. Append or create a copy")


def publish_definition(template: UObject | None, mesh: UObject, name: str, parts: list[FragmentRange]) -> UObject:
    """Construct an independent or template-derived definition with the final ranges."""
    info, bounds = definition_values(template, parts)
    if mesh.Outer is None:
        raise RuntimeError(f"A mesh must have an outer Package: {mesh}")
    if template is None:
        definition = objects.construct("GearboxFramework.GestaltSkeletalMeshDefinition", mesh.Outer, name)
        definition.bStaticMeshRigidBodyMesh = False
        definition.GestaltSocketMappings = []
        definition.GestaltAccessoryNames = []
    else:
        objects.copy_sockets(template.GestaltSkeletalMesh, mesh)
        definition = objects.clone(template, mesh.Outer, name=name)
        names = check_names(parts)
        definition.GestaltSocketMappings = [
            deepcopy(row)
            for row in template.GestaltSocketMappings
            if str(row.SkeletalMeshFragmentName).casefold() in names
        ]
        definition.GestaltAccessoryNames = [
            deepcopy(row)
            for row in template.GestaltAccessoryNames
            if str(row.SkeletalMeshFragmentName).casefold() in names
        ]
    definition.GestaltInfos = [info]
    definition.GestaltPartBounds = bounds
    definition.GestaltSkeletalMesh = mesh
    return definition
