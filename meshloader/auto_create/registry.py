from collections import Counter
from collections.abc import Callable
from dataclasses import dataclass, replace
from pathlib import Path
from typing import TYPE_CHECKING

from unrealsdk import logging

from meshloader.api import load, reload
from meshloader.manifests import CREATE_MESHES, MeshManifest, load_manifest, manifest_files

from .models import ManifestResult, ManifestStatus

if TYPE_CHECKING:
    from unrealsdk.unreal import UObject


def _definitions(directory: str | Path) -> tuple[list[MeshManifest], list[ManifestResult]]:
    valid = []
    results = []
    for file in manifest_files(directory):
        try:
            valid.append(load_manifest(file))
        except (OSError, ValueError, TypeError) as error:
            logging.error(f"{file}: {error}")
            results.append(ManifestResult(str(file), "", "invalid"))
    return valid, results


def load_manifests(directory: str | Path = CREATE_MESHES) -> list[MeshManifest]:
    """Parse recursive TOML manifests and log invalid definitions once per scan."""
    return _definitions(directory)[0]


def create_mesh_for_manifest(manifest: MeshManifest) -> UObject:
    """Load one manifest's mesh and return its SDK object without caching."""
    return load(
        manifest.source,
        manifest.name,
        kind=manifest.kind,
        outer=manifest.outer,
        scale=manifest.scale,
        material=manifest.material,
        materials=manifest.materials,
    )


def reload_mesh_for_manifest(manifest: MeshManifest) -> UObject:
    """Reload one library-created mesh and return its SDK object."""
    return reload(
        manifest.source,
        manifest.name,
        kind=manifest.kind,
        outer=manifest.outer,
        scale=manifest.scale,
        material=manifest.material,
        materials=manifest.materials,
    )


@dataclass(frozen=True)
class _LoadedManifest:
    manifest: MeshManifest
    path: str


class ManifestLoader:
    """Cache only successful loads and optionally reload changed manifests.

    Failed calls are logged once and never cached. A failed reload leaves the
    last successful revision unchanged, so another changed-source scan can
    retry it. Destination names are not reserved between attempts.
    """

    def __init__(
        self,
        factory: Callable[[MeshManifest], UObject] = create_mesh_for_manifest,
        reloader: Callable[[MeshManifest], UObject] = reload_mesh_for_manifest,
    ) -> None:
        """Configure creation and reload callbacks with an empty success cache."""
        self.factory = factory
        self.reloader = reloader
        self._loaded: dict[str, _LoadedManifest] = {}

    def create(self, manifest: MeshManifest, *, reload_changed: bool = False) -> ManifestResult:
        """Load one manifest or reuse its last successfully loaded revision."""
        key = str(manifest.manifest_path)
        previous = self._loaded.get(key)
        if previous is not None:
            if previous.manifest == manifest:
                return ManifestResult(key, previous.path, "already_loaded")
            if not reload_changed:
                logging.info(f"{key}: source/settings changed. Use autoload(reload_changed=True) to load changes")
                return ManifestResult(key, previous.path, "skipped")
        replacing = previous is not None and previous.path.casefold() == manifest.object_path.casefold()
        try:
            mesh = (self.reloader if replacing else self.factory)(manifest)
            path = mesh._path_name()
        except Exception as error:  # noqa: BLE001
            logging.error(f"{key}: {error}")
            return ManifestResult(key, manifest.object_path, "failed")
        snapshot = replace(manifest, materials=manifest.materials.copy())
        self._loaded[key] = _LoadedManifest(snapshot, path)
        status: ManifestStatus = "reloaded" if replacing else "created"
        logging.info(f"{status} {path} from {manifest.source}")
        return ManifestResult(key, path, status)

    def scan(
        self,
        directory: str | Path = CREATE_MESHES,
        *,
        reload_changed: bool = False,
    ) -> list[ManifestResult]:
        """Load a deterministic manifest scan and return transient outcomes."""
        manifests, results = _definitions(directory)
        counts = Counter(manifest.object_path.casefold() for manifest in manifests)
        for manifest in manifests:
            if counts[manifest.object_path.casefold()] > 1:
                logging.error(f"{manifest.manifest_path}: duplicate destination {manifest.object_path}")
                results.append(ManifestResult(str(manifest.manifest_path), manifest.object_path, "conflict"))
            else:
                results.append(self.create(manifest, reload_changed=reload_changed))
        return results
