import json
from pathlib import Path

from .models import BufferRevisions, SourceRevision

_MAX_SOURCE_BYTES = (1 << 31) - 1
_MISSING_REVISION = (-1, -1)


def source_revisions(source: Path) -> tuple[SourceRevision, BufferRevisions]:
    """Read source and external-buffer change hints without decoding geometry."""
    stat = source.stat()
    return (stat.st_size, stat.st_mtime_ns), buffer_revisions(source)


def _buffer_path(root: Path, record: object) -> Path | None:
    if not isinstance(record, dict):
        return None
    uri = record.get("uri")
    if not isinstance(uri, str) or not uri or any(character in uri for character in ":\\%?#\0"):
        return None
    path = Path(uri)
    if path.is_absolute() or ".." in path.parts:
        return None
    try:
        path = (root / path).resolve()
    except OSError, RuntimeError:
        return None
    return path if path.is_relative_to(root) else None


def buffer_revisions(source: Path) -> BufferRevisions:
    """Track safe external buffer paths, including currently missing files.
    .
        Missing or inaccessible paths receive a sentinel so later appearance is detected.
        Unused buffer changes can conservatively trigger a reload.
    """
    if source.suffix.lower() != ".gltf":
        return ()
    if source.stat().st_size > _MAX_SOURCE_BYTES:
        return ()
    with source.open("rb") as stream:
        data = stream.read(_MAX_SOURCE_BYTES + 1)
    if len(data) > _MAX_SOURCE_BYTES:
        return ()
    try:
        document = json.loads(data)
    except ValueError, RecursionError:
        return ()
    if not isinstance(document, dict) or not isinstance(document.get("buffers", []), list):
        return ()
    paths: set[Path] = set()
    for record in document.get("buffers", []):
        path = _buffer_path(source.parent, record)
        if path is not None:
            paths.add(path)
    revisions = []
    for path in sorted(paths):
        try:
            stat = path.stat()
            revision = stat.st_size, stat.st_mtime_ns
        except OSError:
            revision = _MISSING_REVISION
        revisions.append((str(path), *revision))
    return tuple(revisions)
