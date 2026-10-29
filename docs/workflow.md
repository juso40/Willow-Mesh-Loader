# Workflow

## UEViewer/UModel
[https://www.gildor.org/en/projects/umodel](https://www.gildor.org/en/projects/umodel)  
UEViewer, or umodel as we will call it from now on, can show and export the assets in BL2's `.upk` archives.  
There is quite a lot of documentation already online on how to use this tool, so go and look it up if you need any specific help.  

For the recommended workflow you want to use its `gltf/glb` export format for meshes.  

The base games `.upk` are stored in `Borderlands 2/WillowGame/CookedPCConsole/*`.  
Some of the more interesting packages, may be the vault hunters:
- Zer0 `GD_Assassin_Streaming_SF.upk`
- Salvador `GD_Mercenary_Streaming_SF.upk`
- Maya `GD_Siren_Streaming_SF.upk`
- Axton `GD_Soldier_Streaming_SF.upk`

Or their Heads, which roughly follow the pattern `CD_<class>_Head_<class><number>_SF.upk`.

After exporting all the assets as `gltf` you can start importing them in [Blender](#Blender)

## Blender
[https://www.blender.org/](https://www.blender.org/)

If this is your first time working with blender, close it and watch some tutorials, then come back :P  

Now for real, after starting blender, delete the default cube. And everything else… 

We start by importing a `gltf/glb` from the steps before.  
For that:  
`File > Import > glTF 2.0 (.glb/.gltf)`  
Do not change any of blenders default import settings, they are fine for umodel exported meshes.

> [!NOTE]
> Currently we only support a single mesh or a single skinned mesh per exported glb.

While you can edit essentially everything inside blender, you do want to keep in most cases the same armatures and vertex groups.

When rigging a mesh you want to make sure all vertices are properly weight painted or else it wont import in game.



### Export a StaticMesh
In Object Mode select the one mesh you want to export then go to:  
`File > Export > glTF 2.0 (.glb/.gltf)` with the following settings:

| Setting                                      | Value    | Default |
|----------------------------------------------|----------|---------|
| Include > Selected Objects                   | On       | Off     |
| Transform > +Y Up                            | On       | On      |
| Data > Mesh > UVs                            | On       | On      |
| Data > Mesh > Normals                        | On       | On      |
| Data > Mesh > Tangents                       | On       | Off     |
| Data > Material > Materials                  | Viewport | Export  |
| Data > Shape Keys                            | Off      | On      |
| Data > Armature > Use Rest Position Armature | On       | Off     |
| Animation                                    | Off      | On      |

> [!NOTE]
> As always, make sure all Transforms, e.g. Scale and Rotation in Object Mode are applied before exporting!

### Export a SkeletalMesh
In Object Mode select the mesh you want to export, then Shift Click on its Armature, then go to:  
`File > Export > glTF 2.0 (.glb/.gltf)` with the following settings:

| Setting                                      | Value    | Default |
|----------------------------------------------|----------|---------|
| Include > Selected Objects                   | On       | Off     |
| Transform > +Y Up                            | On       | On      |
| Data > Mesh > UVs                            | On       | On      |
| Data > Mesh > Normals                        | On       | On      |
| Data > Mesh > Tangents                       | On       | Off     |
| Data > Material > Materials                  | Viewport | Export  |
| Data > Shape Keys                            | Off      | On      |
| Data > Armature > Use Rest Position Armature | On       | Off     |
| Animation                                    | Off      | On      |

> [!NOTE] 
> As always, make sure all Transforms, e.g. Scale and Rotation in Object Mode are applied before exporting!

> [!NOTE] 
> If the SkeletalMesh/Skin will be in place of an original SkeletalMesh you do need to keep **at least** the same bones and vertex groups in tact! And all vertices need to be weight painted or else blender will export a second empty root skin, which we do not support.


## Loading in Game
Congratz! You made it this far and blender wasn't that scary, was it?

To load your exported mesh into the game you have essentially 2 ways of doing so. One is a bit more straightforward but limited, the other has more control.  
We start with the `.toml` manifest import.

`meshloader` has a folder called `create_mesh` inside this folder it scans recursively for all `.toml` files.
So you could simply create a folder for your new mesh, where you place the exported `.glb` and create a `.toml`

The `.toml` should have the following structure:
```toml
source = "coolMesh.glb"  # relative to this toml
name = "MyCoolMesh"  # This will create Static/SkeletalMesh'Transient.MyCoolMesh'
kind = "auto"  # either, auto, static or skeletal
scale = 100.0  # if you followed everything so far, keep it at 100
```

Now the library will automatically create a new `StaticMesh` or `SkeletalMesh` for you!


The other method requires you to use the python api, for that i recommend simply reading my library docstrings for now :)  
ToDo!
