from collections import Counter, defaultdict
from dataclasses import dataclass, fields, replace
from pathlib import Path
from typing import TYPE_CHECKING

from unrealsdk import logging

from meshloader._revisions import source_revisions
from meshloader.assets.meshes import load, reload
from meshloader.manifests import MeshManifest, load_manifest, manifest_files

if TYPE_CHECKING:
    from unrealsdk.unreal import UObject

_DEFAULT_DIRECTORY = Path(__file__).parent / "create_meshes"
_TRACKING_FIELDS = {"manifest_path", "source_revision", "buffer_revisions"}


def _definition(file: Path) -> MeshManifest | None:
    try:
        return load_manifest(file)
    except (OSError, ValueError, TypeError) as error:
        logging.error(f"{file}: {error}")
        return None


def load_manifests(directory: str | Path = _DEFAULT_DIRECTORY) -> list[MeshManifest]:
    """Parse recursive TOML manifests."""
    return [definition for file in manifest_files(directory) if (definition := _definition(file)) is not None]


def _snapshot(manifest: MeshManifest) -> MeshManifest:
    return replace(
        manifest,
        manifest_path=Path(manifest.manifest_path).resolve(),
        source=Path(manifest.source).resolve(),
        materials=manifest.materials.copy(),
    )


def _refresh_source(manifest: MeshManifest) -> MeshManifest:
    revision, buffers = source_revisions(Path(manifest.source))
    return replace(manifest, source_revision=revision, buffer_revisions=buffers)


def _load_mesh(manifest: MeshManifest, *, reload_existing: bool = False) -> UObject:
    operation = reload if reload_existing else load
    return operation(
        manifest.source,
        manifest.name,
        node=manifest.node,
        scene=manifest.scene,
        kind=manifest.kind,
        outer=manifest.outer,
        scale=manifest.scale,
        material=manifest.material,
        materials=manifest.materials,
        umodel=manifest.umodel,
    )


def create_mesh_for_manifest(manifest: MeshManifest) -> UObject:
    """Create a fresh mesh and register its successful manifest for later reload.

    File-backed manifests are reread by autoload(reload_changed=True), including
    paths outside the scan directory. Programmatic differences from the TOML and
    the created destination are retained. Without readable TOML at creation,
    reload uses the supplied settings and refreshes their source revisions.
    """
    manifest = _snapshot(manifest)
    try:
        baseline = load_manifest(manifest.manifest_path)
    except OSError, ValueError, TypeError:
        baseline = None
    manifest = _refresh_source(manifest)
    mesh = _load_mesh(manifest)
    _loader.register(manifest, mesh, baseline=baseline)
    return mesh


@dataclass(frozen=True)
class _LoadedManifest:
    manifest: MeshManifest
    overrides: dict[str, object] | None
    reread: bool


def _revisit(file: Path, entries: list[_LoadedManifest]) -> list[MeshManifest]:
    """Collect current settings without changing meshes or their success cache.

    File-backed entries share one TOML read and retain runtime overrides.
    Programmatic entries retain settings and refresh only source revisions.
    """
    if not entries:
        definition = _definition(file)
        return [] if definition is None else [definition]

    definition = _definition(file) if any(entry.reread for entry in entries) else None
    manifests = []
    for entry in entries:
        candidate = entry.manifest
        if entry.reread:
            if definition is None:
                continue
            candidate = replace(definition, **(entry.overrides or {}))
            if candidate.source == definition.source:
                manifests.append(candidate)
                continue

        try:
            manifests.append(_refresh_source(candidate))
        except (OSError, ValueError, TypeError) as error:
            logging.error(f"{file}: {candidate.object_path}: {error}")
    return manifests


class ManifestLoader:
    """Cache successful mesh loads and revisit their manifests on explicit reload.

    Failed reloads leave the previous successful revision unchanged. Explicit
    creations retain their runtime overrides and destination identity. Automatic
    directory loads can create a new mesh when their authored destination changes.
    """

    def __init__(self) -> None:
        """Initialize an empty successful-load cache."""
        self._loaded: dict[str, _LoadedManifest] = {}

    def _remember(
        self,
        manifest: MeshManifest,
        path: str,
        *,
        overrides: dict[str, object] | None,
        reread: bool,
    ) -> None:
        manifest = _snapshot(manifest)
        if overrides is None:
            for key, entry in tuple(self._loaded.items()):
                if entry.overrides is None and entry.manifest.manifest_path == manifest.manifest_path:
                    del self._loaded[key]
        self._loaded[path.casefold()] = _LoadedManifest(manifest, overrides, reread)

    def register(self, manifest: MeshManifest, mesh: UObject, *, baseline: MeshManifest | None) -> None:
        """Retain a successful explicit creation without constructing another mesh.

        Different destinations from the same manifest are registered independently.
        Mutable material mappings are copied rather than shared with the caller.
        """
        manifest = _snapshot(manifest)
        overrides = {
            field.name: getattr(manifest, field.name)
            for field in fields(MeshManifest)
            if field.name not in _TRACKING_FIELDS
            and (baseline is None or getattr(manifest, field.name) != getattr(baseline, field.name))
        }
        overrides.update(name=manifest.name, outer=manifest.outer)
        self._remember(manifest, mesh._path_name(), overrides=overrides, reread=baseline is not None)

    def create(self, manifest: MeshManifest, *, reload_changed: bool = False) -> None:
        """Load one manifest or reuse its last successfully loaded revision."""
        manifest = _snapshot(manifest)
        key = str(manifest.manifest_path)
        previous = self._loaded.get(manifest.object_path.casefold())
        if previous is not None:
            if previous.manifest.manifest_path != manifest.manifest_path:
                logging.error(f"{key}: destination {manifest.object_path} belongs to {previous.manifest.manifest_path}")
                return
            if previous.manifest == manifest:
                return
            if not reload_changed:
                logging.info(f"{key}: source/settings changed. Use autoload(reload_changed=True) to load changes")
                return
        try:
            mesh = _load_mesh(manifest, reload_existing=previous is not None)
            path = mesh._path_name()
        except Exception as error:  # noqa: BLE001
            logging.error(f"{key}: {manifest.object_path}: {error}")
            return
        self._remember(
            manifest,
            path,
            overrides=previous.overrides if previous is not None else None,
            reread=previous.reread if previous is not None else True,
        )
        action = "reloaded" if previous is not None else "created"
        logging.info(f"{action} {path} from {manifest.source}")

    def scan(
        self,
        directory: str | Path = _DEFAULT_DIRECTORY,
        *,
        reload_changed: bool = False,
    ) -> None:
        """Scan a directory and revisit all registered paths on explicit reload."""
        files = {file.resolve() for file in manifest_files(directory)}
        tracked: dict[Path, list[_LoadedManifest]] = defaultdict(list)
        for entry in self._loaded.values():
            tracked[entry.manifest.manifest_path].append(entry)
        if reload_changed:
            files.update(tracked)
        manifests = []
        for file in sorted(files, key=lambda path: (str(path).casefold(), str(path))):
            manifests.extend(_revisit(file, tracked[file]))
        counts = Counter(manifest.object_path.casefold() for manifest in manifests)
        for manifest in manifests:
            if counts[manifest.object_path.casefold()] > 1:
                logging.error(f"{manifest.manifest_path}: duplicate destination {manifest.object_path}")
            else:
                self.create(manifest, reload_changed=reload_changed)


_loader = ManifestLoader()


def autoload(directory: str | Path = _DEFAULT_DIRECTORY, *, reload_changed: bool = False) -> None:
    """Scan manifests and optionally reload every registered changed mesh.

    Successful `create_mesh_for_manifest` calls register external paths automatically.
    Reload retains their destinations and programmatic overrides. Outcomes are
    logged, not returned. Textures and non-manifest loads are not included.
    """
    _loader.scan(directory, reload_changed=reload_changed)
