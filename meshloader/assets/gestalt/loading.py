from collections.abc import Sequence
from pathlib import Path
from typing import TYPE_CHECKING, cast

import unrealsdk

from meshloader import validate
from meshloader.defaults import DEFAULT_MATERIAL, DEFAULT_SCALE
from meshloader.models import MaterialMapping, SkeletalInspection
from meshloader.prepared import PreparedGestalt
from meshloader.sources import open_source

from .definition import GestaltAsset
from .inspection import GestaltFragmentInfo, GestaltInspection
from .metadata import GestaltMetadata, load_metadata

if TYPE_CHECKING:
    from unrealsdk.unreal import UObject


def inspect_gestalt(source: str | Path, *, node: str, scene: str, umodel: bool = True) -> GestaltInspection:
    """Inspect sidecar-checked geometry and mapped ranges without a live template."""
    metadata = load_metadata(source)
    prepared = open_source(metadata.source).prepare_mesh(
        node=node,
        scene=scene,
        kind="skeletal",
        scale=metadata.scale,
        umodel=umodel,
    )
    info = cast(SkeletalInspection, prepared.metadata)
    parts = prepared.source_fragments
    if {part.name.casefold() for part in parts} != {name.casefold() for name in metadata.fragment_names}:
        raise ValueError("GLB fragment tags do not match the Gestalt sidecar")
    cursor = 0
    for part in parts:
        if part.first_index != cursor or part.triangle_count <= 0:
            raise ValueError("Gestalt tags must cover every triangle exactly once")
        cursor += part.triangle_count * 3
    if cursor != info.stats.triangle_count * 3 or info.uv_channels != metadata.uv_channels:
        raise ValueError("Gestalt triangle coverage or UV count does not match the export metadata")
    sections = info.source_sections
    slots = [section.source_material for section in sections]
    if any(slot is None or slot >= len(metadata.material_slots) for slot in slots):
        raise ValueError("GLB materials do not match the Gestalt template slot map")
    mapped = []
    for part in parts:
        if not 0 <= part.source_section < len(sections):
            raise ValueError("invalid final fragment section")
        source_slot = cast(int, slots[part.source_section])
        mapped.append(
            GestaltFragmentInfo(
                name=part.name,
                source_section=part.source_section,
                source_material=source_slot,
                material_slot=metadata.material_slots[source_slot],
                first_index=part.first_index,
                triangle_count=part.triangle_count,
                bounds=part.bounds,
            ),
        )
    return GestaltInspection(info, tuple(mapped), metadata.template_mesh, metadata.template_definition)


def _material_paths(mesh: UObject, metadata: GestaltMetadata) -> list[str]:
    materials = list(mesh.Materials)
    if not materials or any(slot >= len(materials) for slot in metadata.material_slots):
        raise ValueError("Exported material slots exceed the template mesh's materials")
    return [DEFAULT_MATERIAL if material is None else material._path_name() for material in materials]


def prepare_gestalt(  # noqa: PLR0913
    source: str | Path,
    *,
    scene: str,
    node: str | None = None,
    nodes: Sequence[str] | None = None,
    template: str | None = None,
    material: str = DEFAULT_MATERIAL,
    materials: MaterialMapping | None = None,
    scale: float | None = None,
    umodel: bool = True,
) -> PreparedGestalt:
    """Prepare a tagged add-on export or named parts from an ordinary glTF/GLB.

    A matching sidecar supplies template provenance and original slot mappings.
    Without a sidecar, nodes become named fragments. Omitting the template retains
    a new skeleton. Omitted node selectors include every mesh in the named scene.
    """
    if node is not None and nodes is not None:
        raise ValueError("Specify node or nodes, not both")
    selected = [node] if node is not None else nodes
    template = None if template is None else validate.object_path(template)
    source = Path(source).expanduser().resolve(strict=True)
    if not source.with_suffix(".gestalt.json").is_file():
        original = None if template is None else unrealsdk.find_object("GestaltSkeletalMeshDefinition", template)
        return open_source(source).prepare_gestalt_parts(
            scene=scene,
            nodes=selected,
            template=original,
            material=material,
            materials=materials,
            scale=DEFAULT_SCALE if scale is None else scale,
            umodel=umodel,
        )
    metadata = load_metadata(source)
    original = unrealsdk.find_object("GestaltSkeletalMeshDefinition", template or metadata.template_definition)
    template_mesh = original.GestaltSkeletalMesh
    if template_mesh._path_name().casefold() != metadata.template_mesh.casefold():
        raise ValueError("The loaded template mesh does not match the source dump")
    document = open_source(metadata.source)
    if selected is None:
        scene_id = validate.source_index(document.catalog.scenes, scene, "scene")
        selected = [mesh.name for mesh in document.catalog.meshes if scene_id in mesh.scene_ids]
        if any(name is None for name in selected):
            raise ValueError("Gestalt mesh nodes require authored names")
    if isinstance(selected, (str, bytes)):
        raise TypeError("nodes must be a sequence of mesh node names, not a single string")
    if len(selected) != 1:
        raise ValueError("A sidecar export requires one combined mesh node")
    paths = _material_paths(template_mesh, metadata)
    mapping = validate.material_mapping(materials) or {}
    if any(slot is None or not 0 <= slot < len(metadata.material_slots) for slot in mapping):
        raise ValueError("Sidecar material mappings require exported source material IDs")
    for source_slot, native_slot in enumerate(metadata.material_slots):
        fallback = paths[native_slot] if material == DEFAULT_MATERIAL else validate.object_path(material)
        paths[native_slot] = mapping.get(source_slot, fallback)
    return document.prepare_gestalt(
        node=selected[0],
        scene=scene,
        template=original,
        fragment_names=metadata.fragment_names,
        materials=paths,
        material_slots=metadata.material_slots,
        uv_channels=metadata.uv_channels,
        scale=metadata.scale if scale is None else scale,
        umodel=umodel,
    )


def load_gestalt(  # noqa: PLR0913
    source: str | Path,
    name: str,
    *,
    scene: str,
    node: str | None = None,
    nodes: Sequence[str] | None = None,
    outer: str | None = None,
    mesh_name: str | None = None,
    template: str | None = None,
    material: str = DEFAULT_MATERIAL,
    materials: MaterialMapping | None = None,
    scale: float | None = None,
    umodel: bool = True,
) -> GestaltAsset:
    """Create a new mesh/definition pair, optionally using an independent skeleton.

    Template-backed exports copy sockets and applicable accessory mappings. An
    independent source needs caller-authored sockets, animation and weapon-type
    setup. Existing weapons are not rebuilt.
    """
    prepared = prepare_gestalt(
        source,
        node=node,
        nodes=nodes,
        scene=scene,
        template=template,
        material=material,
        materials=materials,
        scale=scale,
        umodel=umodel,
    )
    return prepared.create(name, outer=outer, mesh_name=mesh_name)


def load_into_gestalt(  # noqa: PLR0913
    source: str | Path,
    path: str,
    *,
    scene: str,
    node: str | None = None,
    nodes: Sequence[str] | None = None,
    material: str = DEFAULT_MATERIAL,
    materials: MaterialMapping | None = None,
    scale: float | None = None,
    umodel: bool = True,
) -> GestaltAsset:
    """Replace a loaded master's geometry and Gestalt tables without changing identities.

    The path identifies a GestaltSkeletalMeshDefinition, not its SkeletalMesh.
    The existing skeleton is preserved. A sidecar retains original material-slot
    provenance when rebuilding a split UModel export.
    """
    return prepare_gestalt(
        source,
        node=node,
        nodes=nodes,
        scene=scene,
        material=material,
        materials=materials,
        scale=scale,
        umodel=umodel,
    ).load_into(path)


def update_gestalt(  # noqa: PLR0913
    source: str | Path,
    path: str,
    *,
    scene: str,
    nodes: Sequence[str] | None = None,
    material: str | None = None,
    materials: MaterialMapping | None = None,
    scale: float = DEFAULT_SCALE,
    umodel: bool = True,
) -> GestaltAsset:
    """Replace matching named fragments and refresh already assembled components.

    The path identifies a GestaltSkeletalMeshDefinition. Unknown exact names
    are refused and omitted fragments remain. Existing material assignments
    are retained unless material or materials supplies incoming assignments.
    Current part selections, sockets, accessory matrices and animation state
    remain intact. The operation requires a compatible skeleton, an exclusive
    master and one CPU-readable LOD0. A sidecar is unnecessary.
    """
    return (
        open_source(source)
        .prepare_gestalt_parts(
            scene=scene,
            nodes=nodes,
            material=DEFAULT_MATERIAL if material is None else material,
            materials=materials,
            scale=scale,
            umodel=umodel,
        )
        .update_into(path, preserve_materials=material is None and materials is None)
    )


def append_gestalt(  # noqa: PLR0913
    source: str | Path,
    path: str,
    *,
    scene: str,
    nodes: Sequence[str] | None = None,
    material: str = DEFAULT_MATERIAL,
    materials: MaterialMapping | None = None,
    scale: float = DEFAULT_SCALE,
    umodel: bool = True,
) -> GestaltAsset:
    """Append new named parts to the currently loaded master in place.

    The path identifies a GestaltSkeletalMeshDefinition. All mesh nodes in the
    named scene are included unless nodes selects an explicit nonempty list.
    Names must not collide with existing fragments. A sidecar is unnecessary.
    Parts require the master's compatible skeleton. Existing ranges, material
    slots and earlier additions are preserved. Assembled weapons are unchanged.
    """
    return (
        open_source(source)
        .prepare_gestalt_parts(
            scene=scene,
            nodes=nodes,
            material=material,
            materials=materials,
            scale=scale,
            umodel=umodel,
        )
        .append_into(path)
    )
