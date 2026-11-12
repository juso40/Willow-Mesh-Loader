from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import TYPE_CHECKING

from meshloader.defaults import DEFAULT_SAMPLE_RATE, DEFAULT_SCALE
from meshloader.models import AnimationInspection
from meshloader.sources import Source, open_source

if TYPE_CHECKING:
    from unrealsdk.unreal import UObject


def inspect_animations(  # noqa: PLR0913
    source: str | Path | Source,
    *,
    scale: float = DEFAULT_SCALE,
    sample_rate: float = DEFAULT_SAMPLE_RATE,
    umodel: bool = True,
    skin: str,
    clips: Sequence[str],
    scene: str,
) -> AnimationInspection:
    """Inspect selected clip names, timing and reference bones.

    Explicit skin, scene and clip names require exact, unique catalog matches. Mesh
    geometry is not decoded. Channels are uniformly sampled at approximately
    ``sample_rate``. ``umodel`` defaults to PSK-workflow bone frames and
    must reflect the animation source's workflow. False preserves glTF frames.
    """
    return (
        open_source(source)
        .prepare_animations(
            skin=skin,
            clips=clips,
            scene=scene,
            scale=scale,
            sample_rate=sample_rate,
            umodel=umodel,
        )
        .metadata
    )


def load_animations(  # noqa: PLR0913
    source: str | Path | Source,
    name: str,
    *,
    mesh: str | UObject | None = None,
    outer: str | None = None,
    scale: float = DEFAULT_SCALE,
    sample_rate: float = DEFAULT_SAMPLE_RATE,
    umodel: bool = True,
    skin: str,
    clips: Sequence[str],
    scene: str,
    sequence_names: Mapping[str, str] | None = None,
) -> UObject:
    """Create an AnimSet and populate selected clips.

    Tracks use the selected source skin's bone names and order by default.
    An explicit mesh requires matching bone counts, names and direct parents,
    then supplies the track order. Reference poses may differ, but sampled local
    poses are not retargeted or corrected. SequenceNames retain the authored glTF names.
    ``sequence_names`` changes child UObject names, not SequenceNames.
    Skin, scene and clip names require exact, unique matches.
    Existing objects are not replaced and component playback is unchanged.
    """
    prepared = open_source(source).prepare_animations(
        skin=skin,
        clips=clips,
        scene=scene,
        scale=scale,
        sample_rate=sample_rate,
        umodel=umodel,
    )
    return prepared.create(name, mesh=mesh, outer=outer, sequence_names=sequence_names)


def load_into_animation(  # noqa: PLR0913
    source: str | Path | Source,
    path: str,
    *,
    mesh: str | UObject,
    clip: str,
    scale: float = DEFAULT_SCALE,
    sample_rate: float = DEFAULT_SAMPLE_RATE,
    umodel: bool = True,
    skin: str,
    scene: str,
) -> UObject:
    """Replace one loaded AnimSequence while preserving its identity and references.

    ``clip`` selects an exact, unique glTF animation name. The owning AnimSet's
    track order and translation policy remain unchanged. Additive sequences and
    existing curve tracks are refused. Existing notifies require an unchanged
    duration. The caller stops playback users before replacement and reinitializes
    their playback state afterwards.
    """
    prepared = open_source(source).prepare_animations(
        skin=skin,
        clips=[clip],
        scene=scene,
        scale=scale,
        sample_rate=sample_rate,
        umodel=umodel,
    )
    return prepared.load_into(path, mesh=mesh)
