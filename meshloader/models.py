"""Describe import arguments and explicit inspection snapshots."""

from typing import Literal, NotRequired, TypedDict

type MeshKind = Literal["auto", "static", "skeletal"]
type MaterialMapping = dict[int | None, str]
type SourceRevision = tuple[int, int]
type BufferRevisions = tuple[tuple[str, int, int], ...]
type JsonValue = bool | int | float | str | list["JsonValue"] | dict[str, "JsonValue"] | None


class BoundsInfo(TypedDict):
    """Describe local-space box and sphere bounds."""

    origin: list[float]
    box_extent: list[float]
    sphere_radius: float


class SourceSection(TypedDict):
    """Describe one original glTF material partition."""

    section_index: int
    source_material: int | None
    first_index: int
    triangles: int
    min_vertex_index: NotRequired[int]
    max_vertex_index: NotRequired[int]


class ReferenceBone(TypedDict):
    """Describe one reference-pose bone and its local transform."""

    name: str
    parent: int | None
    translation: list[float]
    rotation_xyzw: list[float]


class FragmentInfo(TypedDict):
    """Describe a named index range with complete fragment reference-pose bounds."""

    name: str
    material_index: int
    first_index: int
    triangles: int
    bounds: BoundsInfo


class GeometryInspection(TypedDict):
    """Describe common source geometry and prepared payload metadata."""

    vertices: int
    triangles: int
    scene: int
    instances: int
    primitives: int
    sections: list[SourceSection]
    scale: float
    coordinates: str
    warnings: list[str]
    bounds: BoundsInfo
    payload_bytes: int


_StaticIdentity = TypedDict("_StaticIdentity", {"kind": Literal["static"], "class": Literal["StaticMesh"]})
_SkeletalIdentity = TypedDict("_SkeletalIdentity", {"kind": Literal["skeletal"], "class": Literal["SkeletalMesh"]})


class StaticInspection(GeometryInspection, _StaticIdentity):
    """Describe a prepared static source without constructing a mesh."""

    source_material: int | None


class SkeletalInspection(GeometryInspection, _SkeletalIdentity):
    """Describe a prepared skin, skeleton and fragment layout."""

    bones: list[ReferenceBone]
    palette: list[int]
    active_bones: list[int]
    required_bones: list[int]
    max_bone_influences: int
    tex_coords: int
    index_stride: int
    gestalt_parts: list[FragmentInfo]


type MeshInspection = StaticInspection | SkeletalInspection
