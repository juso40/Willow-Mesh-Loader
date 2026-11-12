from collections.abc import Mapping
from typing import TYPE_CHECKING, cast

import unrealsdk

from meshloader import objects, validate
from meshloader.assets.gestalt.definition import (
    GestaltAsset,
    check_names,
    definition_values,
    publish_definition,
    require_exclusive_mesh,
    template_parts,
)
from meshloader.models import (
    AnimationClipInfo,
    AnimationInspection,
    FragmentRange,
    MeshInspection,
    MeshLayout,
    SkeletalInspection,
    SkeletalLayout,
    SourceFragmentRange,
)

if TYPE_CHECKING:
    from unrealsdk.unreal import UObject

    from meshloader._meshloader import PreparedAnimationSet as NativeAnimations
    from meshloader._meshloader import PreparedMesh as NativeMesh


class PreparedMesh:
    """Hold validated ordinary mesh data before constructing an engine object."""

    def __init__(self, native: NativeMesh) -> None:
        self._native = native

    @property
    def metadata(self) -> MeshInspection:
        """Return an independent inspection snapshot of the prepared geometry."""
        return self._native.metadata

    @property
    def class_name(self) -> str:
        """Return the construction class without materializing mesh inspection."""
        return self._native.class_name

    @property
    def warnings(self) -> list[str]:
        """Return detached source preparation diagnostics."""
        return self._native.warnings

    @property
    def layout(self) -> MeshLayout:
        """Return explicit encoded index and skinning layout diagnostics."""
        return self._native.layout

    @property
    def source_fragments(self) -> list[SourceFragmentRange]:
        """Return tagged ranges using source-section ordinals, not template slots."""
        return self._native.source_fragments

    def populate_new(self, mesh: str | UObject) -> UObject:
        """Populate an empty, unattached, exact-class mesh constructed by Python.

        Initialized assets use replacement instead. Partial native failures are
        not rolled back.
        """
        return objects.finish(self._native.populate_new(objects.path(mesh)))

    def create(self, name: str, *, outer: str | None = None) -> UObject:
        """Construct a mesh and populate it with the prepared geometry."""
        name, outer = validate.target(name, outer)
        mesh = objects.construct(f"Engine.{self.class_name}", objects.package(outer), name)
        return self.populate_new(mesh)

    def reload(self, mesh: str | UObject) -> UObject:
        """Replace library-owned geometry while retaining the mesh identity."""
        return objects.finish(self._native.reload(objects.path(mesh)))

    def load_into(self, mesh: str | UObject) -> UObject:
        """Replace existing geometry under the skeleton and collision restrictions."""
        return objects.finish(self._native.load_into(objects.path(mesh)))


class PreparedAnimationSet:
    """Hold selected clips and their workflow-converted reference skeleton."""

    def __init__(self, native: NativeAnimations) -> None:
        self._native = native

    @property
    def metadata(self) -> AnimationInspection:
        """Return an independent snapshot of the prepared clip names and timing."""
        return self._native.metadata

    @property
    def clips(self) -> list[AnimationClipInfo]:
        """Return ordered clip descriptors without copying the reference skeleton."""
        return self._native.clips

    @property
    def warnings(self) -> list[str]:
        """Return detached source preparation diagnostics."""
        return self._native.warnings

    def check_mesh(self, mesh: str | UObject) -> None:
        """Check target bone counts, names and parents."""
        self._native.check_mesh(objects.path(mesh))

    def populate_new(
        self,
        anim_set: str | UObject,
        sequences: Mapping[str, str | UObject],
        *,
        mesh: str | UObject | None = None,
    ) -> UObject:
        """Populate an empty AnimSet and its distinct empty child sequences.

        Sequence keys cover exactly the prepared clip names. Tracks use the source
        skin's bone order unless an explicit matching mesh supplies another order.
        This installs tracks, codecs and lookup caches without attaching or playing the set.
        """
        clips = self.clips
        names = {clip.name for clip in clips}
        if any(not isinstance(key, str) for key in sequences):
            raise TypeError("sequences keys must be prepared clip names")
        if set(sequences) != names:
            raise ValueError("sequences must map every prepared clip name to exactly one destination")
        paths = [objects.path(sequences[clip.name]) for clip in clips]
        mesh_path = None if mesh is None else objects.path(mesh)
        outcome = self._native.populate_new(objects.path(anim_set), paths, mesh_path)
        return objects.finish(outcome, "AnimSet")

    def create(
        self,
        name: str,
        *,
        mesh: str | UObject | None = None,
        outer: str | None = None,
        sequence_names: Mapping[str, str] | None = None,
    ) -> UObject:
        """Construct an AnimSet and selected child sequences, then populate them.

        Tracks use the source skin's bone names and order unless an explicit mesh
        supplies a validated target order. Optional child UObject names are keyed by
        prepared clip name. SequenceName always retains the clip name.
        """
        name, outer = validate.target(name, outer)
        clips = self.clips
        names = {clip.name: clip.name for clip in clips}
        if sequence_names is not None:
            if any(key not in names for key in sequence_names):
                raise ValueError("sequence_names keys must match prepared clip names exactly")
            names.update(sequence_names)
        for child_name in names.values():
            validate.object_name(child_name)
        if len({value.casefold() for value in names.values()}) != len(names):
            raise ValueError("child sequence UObject names must be distinct")
        if mesh is not None:
            self.check_mesh(mesh)
        anim_set = objects.construct("AnimSet", objects.package(outer), name)
        sequences = {
            clip_name: objects.construct("AnimSequence", anim_set, child) for clip_name, child in names.items()
        }
        return self.populate_new(anim_set, sequences, mesh=mesh)

    def load_into(self, sequence: str | UObject, *, mesh: str | UObject) -> UObject:
        """Replace one loaded sequence from an exactly one-clip prepared payload.

        The caller stops playback users before replacement and reinitializes their
        playback state afterwards. Notify, additive and curve restrictions apply.
        """
        return objects.finish(self._native.load_into(objects.path(sequence), objects.path(mesh)), "AnimSequence")


class PreparedGestalt:
    """Hold named skeletal geometry for Gestalt creation, replacement or append.

    Template identities are paths rather than retained UObject wrappers. Generic
    mesh replacement remains unavailable because it cannot update Gestalt tables.
    """

    def __init__(self, native: NativeMesh, template: UObject | None = None) -> None:
        self._native = native
        self._template = None if template is None else objects.path(template)
        self._names = check_names(native.parts)

    @property
    def metadata(self) -> SkeletalInspection:
        """Return the prepared skeleton and portable payload statistics."""
        return cast(SkeletalInspection, self._native.metadata)

    @property
    def layout(self) -> SkeletalLayout:
        """Return GPU diagnostics in template bone order, or imported order without one."""
        return cast(SkeletalLayout, self._native.layout)

    @property
    def parts(self) -> list[FragmentRange]:
        """Return standalone material slots and full named-fragment bounds."""
        return self._native.parts

    @property
    def warnings(self) -> list[str]:
        """Return detached source preparation diagnostics."""
        return self._native.warnings

    def create(
        self,
        name: str,
        *,
        outer: str | None = None,
        mesh_name: str | None = None,
    ) -> GestaltAsset:
        """Construct a fresh mesh and definition using the prepared template mapping.

        The definition name accepts a qualified path. The optional mesh name is
        a simple sibling name and defaults to the definition name plus ``_Mesh``.
        """
        name, outer = validate.target(name, outer)
        mesh_name = validate.object_name(mesh_name if mesh_name is not None else f"{name}_Mesh")
        if name.casefold() == mesh_name.casefold():
            raise ValueError("Mesh and definition names must be distinct")
        objects.require_unique(f"{outer}.{name}")
        template = (
            None if self._template is None else unrealsdk.find_object("GestaltSkeletalMeshDefinition", self._template)
        )
        mesh = objects.construct("Engine.SkeletalMesh", objects.package(outer), mesh_name)
        mesh = objects.finish(self._native.populate_new(objects.path(mesh)))
        definition = publish_definition(template, mesh, name, self.parts)
        return GestaltAsset(mesh, definition)

    def load_into(self, path: str) -> GestaltAsset:
        """Replace a loaded master's geometry and part tables while preserving both identities.

        The skeleton and sockets remain unchanged. Shared-master definitions are
        refused because their ranges would become invalid. Assembled weapons are
        not rebuilt. Generic reload and automatic rollback remain unavailable.
        """
        definition = unrealsdk.find_object("GestaltSkeletalMeshDefinition", validate.object_path(path))
        require_exclusive_mesh(definition)
        info, bounds = definition_values(definition, self.parts)
        names = self._names
        mappings = [
            row for row in definition.GestaltSocketMappings if str(row.SkeletalMeshFragmentName).casefold() in names
        ]
        accessories = [
            row for row in definition.GestaltAccessoryNames if str(row.SkeletalMeshFragmentName).casefold() in names
        ]
        outcome = self._native.load_into_gestalt(objects.path(definition.GestaltSkeletalMesh))
        mesh = objects.finish(outcome, "SkeletalMesh")
        definition.GestaltInfos = [info]
        definition.GestaltPartBounds = bounds
        definition.GestaltSocketMappings = mappings
        definition.GestaltAccessoryNames = accessories
        return GestaltAsset(mesh, definition)

    def update_into(self, path: str, *, preserve_materials: bool = True) -> GestaltAsset:
        """Replace existing named fragments and refresh current components without changing selections.

        Names match exactly. Unknown names are refused and omitted fragments remain.
        Materials stay assigned to their existing slots by default. Disabling
        preservation adds the prepared material assignments in fresh slots.
        Shared masters, incompatible skeletons and multiple LODs are refused.
        The operation has no automatic rollback.
        """
        definition = unrealsdk.find_object("GestaltSkeletalMeshDefinition", validate.object_path(path))
        outcome = self._native.update_gestalt(objects.path(definition), preserve_materials=preserve_materials)
        return GestaltAsset(objects.finish(outcome, "SkeletalMesh"), definition)

    def append_into(self, path: str) -> GestaltAsset:
        """Append new named fragments to the current master without moving existing ranges.

        Duplicate identities are refused. Existing material slots and fragment
        tables are retained, including earlier mods' additions. Native geometry
        is read at application time, not cached during source preparation.
        """
        definition = unrealsdk.find_object("GestaltSkeletalMeshDefinition", validate.object_path(path))
        existing = set(template_parts(definition))
        existing.update(str(row.SkeletalMeshFragmentName).casefold() for row in definition.GestaltPartBounds)
        if len(definition.GestaltInfos) != 1:
            raise ValueError("Gestalt append supports a single LOD0 table")
        duplicates = self._names & existing
        if duplicates:
            raise ValueError(f"Gestalt fragment identities already exist: {', '.join(sorted(duplicates))}")
        info, bounds = definition_values(None, self.parts)
        old_parts = list(definition.GestaltInfos[0].Parts)
        old_bounds = list(definition.GestaltPartBounds)
        outcome = self._native.append_gestalt(objects.path(definition.GestaltSkeletalMesh))
        for row, part in zip(info.Parts, outcome.parts, strict=True):
            row.FirstIndex = part.first_index
            row.MaterialIndex = part.material_slot
        info.Parts = old_parts + list(info.Parts)
        definition.GestaltInfos = [info]
        definition.GestaltPartBounds = old_bounds + bounds
        return GestaltAsset(objects.finish(outcome, "SkeletalMesh"), definition)
