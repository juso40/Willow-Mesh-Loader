from pathlib import Path

from meshloader.manifests import CREATE_MESHES

from .models import ManifestResult
from .registry import ManifestLoader

_loader = ManifestLoader()


def autoload(directory: str | Path = CREATE_MESHES, *, reload_changed: bool = False) -> list[ManifestResult]:
    """Load pending manifests and optionally reload changed successful sources."""
    return _loader.scan(directory, reload_changed=reload_changed)
