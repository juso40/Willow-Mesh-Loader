from typing import TYPE_CHECKING, cast

from meshloader._meshloader import get_reference_skeleton as native_get_reference_skeleton

if TYPE_CHECKING:
    from unrealsdk.unreal import UObject

    from meshloader.models import ReferenceBone


def get_reference_skeleton(mesh: UObject) -> list[ReferenceBone]:
    """Read a loaded SkeletalMesh's reference bones in native order.

    Records contain names, parent indices and local reference frames. Root
    parents are None.
    """
    return cast("list[ReferenceBone]", native_get_reference_skeleton(mesh._path_name()))
