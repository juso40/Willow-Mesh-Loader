from dataclasses import dataclass

from meshloader.models import MeshBounds, SkeletalInspection


@dataclass(frozen=True)
class GestaltFragmentInfo:
    """Describe one source range and its sidecar-declared template material slot."""

    name: str
    source_section: int
    source_material: int
    material_slot: int
    first_index: int
    triangle_count: int
    bounds: MeshBounds


@dataclass(frozen=True)
class GestaltInspection:
    """Describe sidecar-checked geometry without a loaded template.

    Geometry bones and GPU palettes retain source order. Expected template paths
    and material slots describe the sidecar, not a validated live engine mesh.
    PreparedGestalt instead exposes template-bound metadata and final parts.
    """

    geometry: SkeletalInspection
    parts: tuple[GestaltFragmentInfo, ...]
    expected_template_mesh: str
    expected_template_definition: str
