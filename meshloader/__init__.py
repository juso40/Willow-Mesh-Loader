from mods_base import Library, build_mod

from ._autoload import autoload, create_mesh_for_manifest, load_manifests
from ._commands import meshloader_command
from .assets.animations import inspect_animations, load_animations, load_into_animation
from .assets.gestalt.definition import GestaltAsset
from .assets.gestalt.inspection import GestaltFragmentInfo, GestaltInspection
from .assets.gestalt.loading import append_gestalt, inspect_gestalt, load_gestalt, load_into_gestalt, prepare_gestalt
from .assets.meshes import inspect, load, reload
from .assets.skeletal_mesh import (
    get_reference_skeleton,
    inspect_skeletal_mesh,
    load_into_skeletal_mesh,
    load_skeletal_mesh,
)
from .assets.static_mesh import inspect_static_mesh, load_into_static_mesh, load_static_mesh
from .manifests import MeshManifest, load_manifest
from .objects import copy_sockets
from .prepared import PreparedAnimationSet, PreparedGestalt, PreparedMesh
from .sources import Source, open_source

inspect_gltf = inspect_static_mesh
inspect_skeletal_gltf = inspect_skeletal_mesh

__all__ = [
    "GestaltAsset",
    "GestaltFragmentInfo",
    "GestaltInspection",
    "MeshManifest",
    "PreparedAnimationSet",
    "PreparedGestalt",
    "PreparedMesh",
    "Source",
    "append_gestalt",
    "autoload",
    "copy_sockets",
    "create_mesh_for_manifest",
    "get_reference_skeleton",
    "inspect",
    "inspect_animations",
    "inspect_gestalt",
    "inspect_gltf",
    "inspect_skeletal_gltf",
    "inspect_skeletal_mesh",
    "inspect_static_mesh",
    "load",
    "load_animations",
    "load_gestalt",
    "load_into_animation",
    "load_into_gestalt",
    "load_into_skeletal_mesh",
    "load_into_static_mesh",
    "load_manifest",
    "load_manifests",
    "load_skeletal_mesh",
    "load_static_mesh",
    "open_source",
    "prepare_gestalt",
    "reload",
]

# The factory discovers package metadata from its calling module.
build_mod(cls=Library, commands=[meshloader_command])
autoload()
