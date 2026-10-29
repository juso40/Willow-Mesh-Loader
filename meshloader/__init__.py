from mods_base import Library, build_mod

from meshloader.api import (
    get_reference_skeleton,
    inspect,
    inspect_gltf,
    inspect_skeletal_gltf,
    load,
    load_into_skeletal_mesh,
    load_into_static_mesh,
    load_skeletal_mesh,
    load_static_mesh,
    reload,
)
from meshloader.auto_create import (
    autoload,
    create_mesh_for_manifest,
    load_manifests,
)
from meshloader.gestalt import GestaltAsset, inspect_gestalt, load_gestalt
from meshloader.manifests import MeshManifest, load_manifest

__all__ = [
    "GestaltAsset",
    "MeshManifest",
    "autoload",
    "create_mesh_for_manifest",
    "get_reference_skeleton",
    "inspect",
    "inspect_gestalt",
    "inspect_gltf",
    "inspect_skeletal_gltf",
    "load",
    "load_gestalt",
    "load_into_skeletal_mesh",
    "load_into_static_mesh",
    "load_manifest",
    "load_manifests",
    "load_skeletal_mesh",
    "load_static_mesh",
    "reload",
]


build_mod(cls=Library)
autoload()
