from copy import deepcopy
from typing import TYPE_CHECKING

import unrealsdk
from mods_base import ObjectFlags

if TYPE_CHECKING:
    from unrealsdk.unreal import UObject, WrappedStruct

    from meshloader._meshloader import FragmentRange

ROOT_SET = 0x4000
ROOT_FLAGS = ROOT_SET | (1 << 34) | (1 << 51)


def template_parts(template: UObject) -> dict[str, WrappedStruct]:
    """Return LOD0 part prototypes indexed by case-insensitive fragment identity."""
    if template.bStaticMeshRigidBodyMesh or template.GestaltSkeletalMesh is None:
        raise ValueError("The template must be a non-rigid-body Gestalt skeletal mesh definition")
    if not template.GestaltInfos or not template.GestaltInfos[0].Parts:
        raise ValueError("The template has no LOD0 Gestalt parts")
    if not template.GestaltPartBounds:
        raise ValueError("The template has no Gestalt bounds prototype")
    return {str(part.SkeletalMeshFragmentName).casefold(): deepcopy(part) for part in template.GestaltInfos[0].Parts}


def definition_values(template: UObject, parts: list[FragmentRange]) -> tuple[WrappedStruct, list[WrappedStruct]]:
    """Build final range and reference-pose bounds tables without changing the template."""
    prototypes = template_parts(template)
    info = deepcopy(template.GestaltInfos[0])
    rows = []
    bounds = {}
    original_bounds = {str(item.SkeletalMeshFragmentName).casefold(): item for item in template.GestaltPartBounds}
    for part in parts:
        key = part.name.casefold()
        row = deepcopy(prototypes[key])
        row.SkeletalMeshFragment = None
        row.MaterialIndex = part.material_index
        row.FirstIndex = part.first_index
        row.NumPrimitives = part.triangles
        rows.append(row)
        if key not in bounds:
            item = deepcopy(original_bounds.get(key, template.GestaltPartBounds[0]))
            item.SkeletalMeshFragmentName = row.SkeletalMeshFragmentName
            values = part.bounds
            for axis, origin, extent in zip("XYZ", values.origin, values.box_extent, strict=True):
                setattr(item.ReferencePoseBounds.Origin, axis, origin)
                setattr(item.ReferencePoseBounds.BoxExtent, axis, extent)
            item.ReferencePoseBounds.SphereRadius = values.sphere_radius
            bounds[key] = item
    info.Parts = rows
    return info, list(bounds.values())


def copy_sockets(template_mesh: UObject, mesh: UObject) -> None:
    """Clone socket UObjects under the fresh mesh rather than sharing mutable originals."""
    sockets = []
    for original in template_mesh.Sockets:
        socket = unrealsdk.construct_object(
            original.Class,
            mesh,
            original.Name,
            flags=ObjectFlags.KEEP_ALIVE | ObjectFlags.ALLOW_CROSS_PACKAGE_REFERENCES,
            template_obj=original,
        )
        if socket is None:
            raise RuntimeError("Socket construction failed")
        sockets.append(socket)
    mesh.Sockets = sockets


def publish_definition(template: UObject, mesh: UObject, name: str, parts: list[FragmentRange]) -> UObject:
    """Construct a rooted definition with final ranges, bounds and copied socket mappings."""
    info, bounds = definition_values(template, parts)
    copy_sockets(template.GestaltSkeletalMesh, mesh)
    definition = unrealsdk.construct_object(
        template.Class,
        mesh.Outer,
        name,
        flags=ObjectFlags.KEEP_ALIVE | ObjectFlags.ALLOW_CROSS_PACKAGE_REFERENCES,
        template_obj=template,
    )
    if definition is None:
        raise RuntimeError("Gestalt definition construction failed")
    definition.GestaltInfos = [info]
    definition.GestaltPartBounds = bounds
    definition.GestaltSocketMappings = deepcopy(template.GestaltSocketMappings)
    definition.GestaltAccessoryNames = deepcopy(template.GestaltAccessoryNames)
    definition.GestaltSkeletalMesh = mesh
    return definition
