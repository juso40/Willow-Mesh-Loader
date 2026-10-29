from dataclasses import dataclass
from typing import Literal

type ManifestStatus = Literal[
    "invalid",
    "conflict",
    "skipped",
    "already_loaded",
    "created",
    "reloaded",
    "failed",
]


@dataclass(frozen=True)
class ManifestResult:
    """Describe one manifest operation without retaining errors or native diagnostics."""

    manifest: str
    path: str
    status: ManifestStatus
