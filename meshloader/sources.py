from collections.abc import Sequence
from functools import cached_property
from pathlib import Path
from typing import TYPE_CHECKING

from meshloader import objects, validate
from meshloader._meshloader import Source as NativeSource
from meshloader.assets.gestalt.definition import template_parts
from meshloader.defaults import DEFAULT_MATERIAL, DEFAULT_SAMPLE_RATE, DEFAULT_SCALE
from meshloader.models import MaterialMapping, MeshKind, SourceCatalog, SourceIdentities
from meshloader.prepared import PreparedAnimationSet, PreparedGestalt, PreparedMesh

if TYPE_CHECKING:
    from unrealsdk.unreal import UObject


class Source:
    """Retain a parsed glTF/GLB document and lazily cached validated buffers.

    Explicit selectors use exact authored names and require one matching entry.
    Missing or ambiguous names raise ValueError.
    A new Source observes changed files, including external buffers.
    """

    def __init__(self, path: str | Path) -> None:
        self._native = NativeSource(str(path))

    @property
    def catalog(self) -> SourceCatalog:
        """Return asset identities without decoding buffers or validating payloads."""
        return self._native.catalog()

    @cached_property
    def _identities(self) -> SourceIdentities:
        return self._native.identities

    def material_slots(self, *, node: str, scene: str, kind: MeshKind = "auto") -> list[int | None]:
        """Return first-use source material IDs without decoding mesh geometry.

        Slots are descriptive, not an admission guarantee or final native draw
        sections. None denotes unassigned faces. Exact global name rules apply.
        The optional kind guards static or skeletal expectations.
        """
        return self._native.material_slots(
            node=validate.source_index(self._identities.meshes, node, "mesh node"),
            scene=validate.source_index(self._identities.scenes, scene, "scene"),
            kind=validate.kind(kind),
        )

    def prepare_mesh(  # noqa: PLR0913
        self,
        *,
        node: str,
        scene: str,
        kind: MeshKind = "auto",
        material: str = DEFAULT_MATERIAL,
        materials: MaterialMapping | None = None,
        scale: float = DEFAULT_SCALE,
        umodel: bool = True,
    ) -> PreparedMesh:
        """Decode a mesh selected by exact node and scene names.

        Static node transforms are baked. Skinned mesh-node transforms are ignored.
        ``umodel=True`` converts PSK-workflow Y-up export bone frames. False
        preserves authored glTF frames. Static geometry is unchanged.
        """
        node_id = validate.source_index(self._identities.meshes, node, "mesh node")
        scene_id = validate.source_index(self._identities.scenes, scene, "scene")
        return PreparedMesh(
            self._native.prepare_mesh(
                node=node_id,
                scene=scene_id,
                kind=validate.kind(kind),
                material=material,
                materials=validate.material_mapping(materials),
                scale=validate.scale(scale),
                umodel=validate.umodel(umodel),
            ),
        )

    def prepare_animations(  # noqa: PLR0913
        self,
        *,
        skin: str,
        clips: Sequence[str],
        scene: str,
        scale: float = DEFAULT_SCALE,
        sample_rate: float = DEFAULT_SAMPLE_RATE,
        umodel: bool = True,
    ) -> PreparedAnimationSet:
        """Sample exactly named clips for a skin without decoding mesh geometry.

        Clip lists are nonempty and distinct. Prepared clips retain document order.
        Channels outside the skin and its ancestors are excluded with a warning.
        ``umodel`` selects the animation source's bone-frame workflow.
        """
        skin_id = validate.source_index(self._identities.skins, skin, "skin")
        scene_id = validate.source_index(self._identities.scenes, scene, "scene")
        if isinstance(clips, (str, bytes)):
            raise TypeError("clips must be a sequence of animation names, not a single string")
        selected = [validate.source_index(self._identities.animations, clip, "animation") for clip in clips]
        if not selected or len(set(selected)) != len(selected):
            raise ValueError("clips must contain distinct animation names and cannot be empty")
        return PreparedAnimationSet(
            self._native.prepare_animations(
                skin=skin_id,
                clips=selected,
                scene=scene_id,
                scale=validate.scale(scale),
                sample_rate=validate.sample_rate(sample_rate),
                umodel=validate.umodel(umodel),
            ),
        )

    def prepare_gestalt_parts(  # noqa: PLR0913
        self,
        *,
        scene: str,
        nodes: Sequence[str] | None = None,
        template: UObject | None = None,
        material: str = DEFAULT_MATERIAL,
        materials: MaterialMapping | None = None,
        scale: float = DEFAULT_SCALE,
        umodel: bool = True,
    ) -> PreparedGestalt:
        """Prepare named nodes as Gestalt fragments, or all mesh nodes in the named scene.

        Untagged geometry uses authored node names as fragment identities. Tagged
        primitives retain their fragment names. All nodes require compatible
        skinned reference skeletons. Omitting the template retains an independent skeleton.
        """
        scene_id = validate.source_index(self._identities.scenes, scene, "scene")
        if nodes is None:
            nodes = [mesh.name for mesh in self.catalog.meshes if scene_id in mesh.scene_ids]
            if any(name is None for name in nodes):
                raise ValueError("Gestalt mesh nodes require authored names")
        if isinstance(nodes, (str, bytes)):
            raise TypeError("nodes must be a sequence of mesh node names, not a single string")
        selected = [validate.source_index(self._identities.meshes, node, "mesh node") for node in nodes]
        if not selected or len(set(selected)) != len(selected):
            raise ValueError("Gestalt nodes must be nonempty and distinct")
        if template is not None:
            template_parts(template)
        native = self._native.prepare_gestalt_parts(
            nodes=selected,
            scene=scene_id,
            template=None if template is None else objects.path(template.GestaltSkeletalMesh),
            material=validate.object_path(material),
            materials=validate.material_mapping(materials),
            scale=validate.scale(scale),
            umodel=validate.umodel(umodel),
        )
        return PreparedGestalt(native, template)

    def prepare_gestalt(  # noqa: PLR0913
        self,
        *,
        node: str,
        scene: str,
        template: UObject,
        fragment_names: Sequence[str],
        materials: Sequence[str],
        material_slots: Sequence[int],
        uv_channels: int,
        scale: float = DEFAULT_SCALE,
        umodel: bool = True,
    ) -> PreparedGestalt:
        """Prepare tagged geometry against a loaded Gestalt definition template.

        Fragment identities, slot mappings and UV count normally come from the
        add-on sidecar. ``prepare_gestalt()`` at the package facade validates that
        pair and resolves the template before calling this lower-level method.
        """
        if isinstance(fragment_names, str) or any(not isinstance(name, str) for name in fragment_names):
            raise TypeError("fragment_names must be a sequence of fragment names")
        template_parts(template)
        native = self._native.prepare_gestalt(
            objects.path(template.GestaltSkeletalMesh),
            fragment_names,
            materials,
            material_slots,
            uv_channels,
            node=validate.source_index(self._identities.meshes, node, "mesh node"),
            scene=validate.source_index(self._identities.scenes, scene, "scene"),
            scale=validate.scale(scale),
            umodel=validate.umodel(umodel),
        )
        return PreparedGestalt(native, template)


def open_source(path: str | Path | Source) -> Source:
    """Open a glTF/GLB source, or preserve an existing source handle."""
    return path if isinstance(path, Source) else Source(path)
