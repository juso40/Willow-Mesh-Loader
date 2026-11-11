# meshloader

Runtime mesh and animation loading for **Borderlands 2**: import glTF/GLB files
as real, game-ready `StaticMesh`, `SkeletalMesh` and `AnimSet` objects, or create
and edit Gestalt weapon masters.

A [pyunrealsdk](https://bl-sdk.github.io/willow2-mod-db/) mod.

> [!NOTE] 
> 
> | [![ai:r](https://github.com/NamtaoProductions/0rwx.com/blob/main/static/32x32/AI-R-32x32.png?raw=true)](https://0rwx.com) [meshloader/](meshloader/) | AI:R I've written the code myself, but used LLMs for architecture suggestions, bug fixes, improvements, and for writing most docs. |
> | ----------------------- |  :------------------------------------------------------------------------------------------------ |
>
> | [![ai:w](https://github.com/NamtaoProductions/0rwx.com/blob/main/static/32x32/AI-W-32x32.png?raw=true)](https://0rwx.com) [native/](native/) | AI:W I've used LLMs for reverse engineering the game and generating large parts of the code. I've read and tested all code.  |
> | ----------------------- |  ------------------------------------------------------------------------------------------------ |





## Install

Unpack `meshloader.zip` into `Binaries/Win32/Plugins/sdk_mods/`. 


## Build

With Docker installed, run from the repository root:

```sh
./scripts/build_windows.sh
```

This builds the 32-bit Windows native module in release mode and generates its type stub,
placing `_meshloader.pyd` and `_meshloader.pyi` in `meshloader/`.

## How to Use

meshloader works as a manifest-based mesh loader and as a Python library for
other mods. If another mod lists meshloader as a requirement, install it and
follow that mod's instructions.


*The following sections are for mod developers.*

### For `blcm` Modders

Place your `.gltf` or `.glb` files and their `.toml` manifests in
`meshloader/create_meshes/`. Subfolders are supported. meshloader scans recursively
for manifests when it starts and creates the described meshes.

For `.gltf` files, include any referenced external `.bin` buffers too.

#### `.toml` Manifest

Each manifest describes one mesh node in one scene:

```toml
source = "character.glb"       # required, relative to this manifest
name = "CustomBody"            # required, destination object name
node = "Body"                  # required, exact mesh-node name in the source
scene = "Scene"                # required, exact scene name in the source, usually "Scene"

# Optional settings, with their defaults shown:
kind = "auto"                  # "auto", "static" or "skeletal"
outer = "Transient"            # destination package, must already be loaded
scale = 100.0                  # metres to centimetres
umodel = true                  # PSK/PSA editing workflow, explained below
material = "EngineMaterials.DefaultMaterial"

# Optional source-material overrides. These materials must already be loaded.
[materials]
0 = "MyPackage.BodyMaterial"
default = "MyPackage.UnassignedMaterial"  # faces without a glTF material
```

For a skinned `Body` node, this creates
`SkeletalMesh'Transient.CustomBody'`. An unskinned node creates a `StaticMesh`.
Use the resulting object path in your mod's mesh assignments.

Source paths are relative to the TOML file, not the working directory. `node`
means the mesh object's node name, not the geometry datablock name. Node and scene
selectors must match exactly and be unique within their source category.

Bare destination names use `Transient`. A qualified name such as
`MyPackage.CustomBody` can select another loaded outer package instead. Omit `outer`
when using a qualified name. Manifests never overwrite existing objects.

Manifests currently only support creating static and skeletal meshes. 
Animations and Gestalt operations use the Python API.

### For Python Developers

You can use the same manifest format from your own mod without copying assets
into meshloader's folder:

```python
from pathlib import Path

from meshloader import create_mesh_for_manifest, load_manifest

manifest_path = Path(__file__).resolve().parent / "meshes" / "body.toml"
body = create_mesh_for_manifest(load_manifest(manifest_path))
```

`create_mesh_for_manifest` returns the created UObject and registers its successful
creation for explicit manifest reload. `load_manifest` only parses the TOML.

To load directly, use `load_static_mesh` or `load_skeletal_mesh`:

```python
import meshloader

source = meshloader.open_source("character.glb")
print([entry.name for entry in source.catalog.meshes])

body = meshloader.load_skeletal_mesh(
    source,
    "CustomBody",
    node="Body",
    scene="Scene",
    materials={0: "MyPackage.BodyMaterial"},
)
```

The load helpers return UObjects. `load` detects static or skeletal geometry automatically. 
`open_source` creates a reusable `Source` object that caches the parsed documents content, allowing you to create multiple objects from a single source isntead of reparsing the file each time. 
A `Source` does not watch for file changes as it reads the sources data only once and caches it.

`Source.prepare_mesh()` separates decoding from UObject creation when you need
inspection or want to create several meshes from one prepared payload. 

#### Materials

Authored glTF shaders, textures, and images are not imported. Source material IDs
and slot order are retained so you can assign loaded Unreal materials instead.

`material` supplies the fallback for unmapped slots. `materials` maps original
glTF material indices to engine material paths. Use `None` as the Python key for
faces without a material, equivalent to `default` in a manifest.

#### Animations

Load named clips into a new `AnimSet` using a mesh as the skeleton reference:

```python
animations = meshloader.load_animations(
    source,
    "CustomAnimations",
    mesh=body,
    skin="BodyRig",
    scene="Scene",
    clips=["Run", "Wave"],
    sample_rate=30.0,
)
```

The source needs a skin and joints, but does not need mesh geometry. Skin, scene
and clip selectors are exact, case-sensitive names. `clips` must be nonempty and
distinct. Loading does not attach the AnimSet or start playback.

Animations are absolute translation/rotation tracks.
The preparation reference must have the same bone count, exact names and direct
parents. Bone order may differ. Reference poses may differ too, but imported
local samples are not corrected for those differences.

#### Gestalt Weapons

Use `append_gestalt` to add named parts to a loaded weapon master without moving
its existing ranges or material slots:

```python
asset = meshloader.append_gestalt(
    "new_parts.glb",
    "Weap_AssaultRifles.GestaltDef_AssaultRifle",
    scene="Scene",
    nodes=["MyMod_Sight", "MyMod_Grip"],
)
```

The target path identifies a `GestaltSkeletalMeshDefinition`, not its mesh.
Parts require a compatible skeleton and names that do not collide with existing
fragments. Omitting `nodes` includes every mesh node in the selected scene.

Use `update_gestalt` to replace existing named fragments without replacing the
rest of the master:

```python
asset = meshloader.update_gestalt(
    "edited_body.glb",
    "Weap_AssaultRifles.GestaltDef_AssaultRifle",
    scene="Scene",
    nodes=["BodyBandit"],
)
```

The incoming `BodyBandit` replaces that exact fragment identity. Unknown names
are rejected and omitted fragments remain. Triangle counts may change. Existing
material assignments are retained unless `material` or `materials` supplies new
assignments. Current components rebuild their cached fragment ranges, socket
mappings and bounds from their saved part selections, including first-person,
third-person and accessory meshes. Weapon construction is not rerun.

`load_into_gestalt` replaces the complete master's geometry and part tables while
preserving its UObject identities. `load_gestalt` creates an independent mesh and
definition pair. Append, complete replacement and independent creation do not
refresh already assembled weapons.


### Blender and UModel Sources

meshloader reads **glTF 2.0 and GLB**, not PSK or PSA files. PSK/PSA are editing
formats that you export to glTF after authoring.

| Source workflow | `umodel` setting |
| --- | --- |
| PSK rig and optional PSA actions, then ordinary Blender **+Y Up** glTF export | `true`, the default |
| Direct UModel glTF, or a Blender glTF import/export round trip | `false` |

In Python, use `umodel=True` or `umodel=False`. The flag selects bone-frame
conversion, not geometry conversion or automatic retargeting. Both workflows
map glTF `(X, Y, Z)` to engine `(X, Z, Y)`, multiplied by `scale`.


### Reloading During Development

Run **`meshloader reload`** in the SDK console, or call
`meshloader.autoload(reload_changed=True)`, to revisit scanned manifests and
successful `create_mesh_for_manifest` registrations, including mod-owned paths.
Changed meshes are updated in place. Failed reloads retain the last successful
revision and do not fall back to fresh creation.

Direct `load` calls are not registered for manifest reload. Use `reload` for
library-created meshes, or `load_into_static_mesh` and `load_into_skeletal_mesh`
for supported existing assets. `load_into_animation` replaces one loaded sequence.

Mesh replacement requires unchanged skeletal names, order, parents and reference
frames. 

## Notes and Limitations

- Only one render LOD is supported. No automatic physics or collision generation.
- Static meshes use UV0 and u16 indices, with at most 65,536 encoded vertices.
  Discarded UV channels do not load buffers or retain seams.
- Skeletal meshes support one to four UV channels, four influences per vertex
  and at most 256 reference bones. Automatic GPU chunks contain at most 75
  weighted bones each. Skeletal indices use u16 or u32 as needed.
- Skeletal binds must be rigid. Genuine animated scale, shear, reflections,
  morphs, cloth and consumed sparse accessors are unsupported.
- Static replacement preserves supported simple collision, but refuses native
  triangle collision trees and unsupported component states.
