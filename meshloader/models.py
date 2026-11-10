from typing import Literal

from ._meshloader import (
    AnimationClipInfo,
    AnimationInspection,
    AnimationOutcome,
    FragmentRange,
    GestaltAppendOutcome,
    MeshBounds,
    MeshOutcome,
    MeshStatistics,
    ReferenceBone,
    SectionVertexRange,
    SkeletalChunkInfo,
    SkeletalInspection,
    SkeletalLayout,
    SourceAnimation,
    SourceCatalog,
    SourceFragmentRange,
    SourceIdentities,
    SourceIdentity,
    SourceMesh,
    SourceMeshIdentity,
    SourceScene,
    SourceSection,
    SourceSelection,
    SourceSkin,
    StaticInspection,
    StaticLayout,
)

type MeshKind = Literal["auto", "static", "skeletal"]
type MaterialMapping = dict[int | None, str]
type SourceRevision = tuple[int, int]
type BufferRevisions = tuple[tuple[str, int, int], ...]
type MeshInspection = StaticInspection | SkeletalInspection
type MeshLayout = StaticLayout | SkeletalLayout

BoundsInfo = MeshBounds
FragmentInfo = FragmentRange

__all__ = [
    "AnimationClipInfo",
    "AnimationInspection",
    "AnimationOutcome",
    "BoundsInfo",
    "BufferRevisions",
    "FragmentInfo",
    "FragmentRange",
    "GestaltAppendOutcome",
    "MaterialMapping",
    "MeshBounds",
    "MeshInspection",
    "MeshKind",
    "MeshLayout",
    "MeshOutcome",
    "MeshStatistics",
    "ReferenceBone",
    "SectionVertexRange",
    "SkeletalChunkInfo",
    "SkeletalInspection",
    "SkeletalLayout",
    "SourceAnimation",
    "SourceCatalog",
    "SourceFragmentRange",
    "SourceIdentities",
    "SourceIdentity",
    "SourceMesh",
    "SourceMeshIdentity",
    "SourceRevision",
    "SourceScene",
    "SourceSection",
    "SourceSelection",
    "SourceSkin",
    "StaticInspection",
    "StaticLayout",
]
