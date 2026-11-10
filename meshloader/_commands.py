import argparse

from mods_base import command

from ._autoload import autoload


@command("meshloader")
def meshloader_command(args: argparse.Namespace) -> None:
    """Reload changed directory-scanned and explicitly registered mesh manifests."""
    if args.action == "reload":
        autoload(reload_changed=True)


meshloader_command.add_argument(
    "action",
    choices=("reload",),
    help="Reload changed scanned and registered mesh manifests, including mod-owned paths. Textures are not reloaded.",
)
