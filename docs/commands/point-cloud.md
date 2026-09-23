# PointCloud

[Command reference](README.md) · [Point-cloud picking](../point-cloud-picking.md)

`PointCloud [UsePointColors=No]` creates one point cloud from selected point
objects and mesh vertices. It does not accept a typed list of coordinates.

```text
Point 1,2,3
Point 4,5,6
SelPt
PointCloud
```

Alternatively, enter `PointCloud`, select point and mesh objects, and press
Enter. This creation picker ignores curves and existing clouds. Esc cancels.
Preselected inputs are traversed in document order; command-first inputs use
pick order. Within each mesh, every stored vertex is retained, including unused
vertices and duplicates. A lone point does not create a cloud.

Consumed point objects are deleted; meshes remain. The output is unselected,
unnamed, ungrouped, and uses the current layer and layer color. Preselected
meshes and unrelated objects retain selection. Command-first selection is
cleared on completion, including a single-point no-op. Creation is one undo
step, restoring source geometry, attributes, group membership, and selection
membership. Batch deletion also restores the removed points' relative pick order;
interleaving with unrelated picks and Rhino Undo pick-order parity are not promised.

## Edit an existing cloud

Select one target cloud, enter `PointCloud Add`, pick point objects or other
point clouds, then press Enter. Esc restores the selection from before the
prompt. You can also preselect the target and point sources and run Add directly.
The selected sources append in selection order and are consumed. If several
clouds are preselected, specify the target object's ID:

```text
PointCloud Add Target=<id>
```

The target retains its ID, attributes, groups, and selection. Meshes are not Add
sources. Add is one undo step.

Remove uses zero-based stored point indices, separated by commas. Removed points
keep their stored order, and duplicate indices count once:

```text
PointCloud Remove Indices=0,3 Output=Points
PointCloud Remove Indices=1,2 Output=PointCloud
```

Select one target cloud before running Remove, or specify `Target=<id>`. The
default output is `Points`. Output objects inherit the target's attributes;
groups are not copied. If all points are removed, the empty source cloud is
deleted. An invalid index leaves the document unchanged. Remove is one undo
step.

## Limits

Per-point colors are not implemented; `UsePointColors=Yes` returns an error.
Remove currently uses typed indices. Viewport point subobject picking and
Rhino's interactive Remove option loop are pending.
Bare `PointCloud` with a preselected cloud asks for an explicit edit action.
Creation still ignores existing cloud inputs. Use `Explode` to extract individual
points from a cloud, or `ExtractPt Output=PointCloud` for supported geometry
extraction workflows. `PointGrid` creates rectangular clouds.

The [Rhino reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/pointcloud.htm)
describes additional cloud editing and color workflows; these are not implied
by the implemented creation path.

## Verification

Eight Rhino `8.32.26160.13001` measurements cover preselection and command-first
point order, duplicates, single-point no-ops, mesh retention, unused vertices,
mixed input, output attributes, and selection. The
[raw response](../../tools/rhino_oracle/observations/point_cloud_command.json)
replays against native command execution. Independent Python checks use explicit
expected point lists and retained source indices. Native tests also cover
undo/redo, group restoration, Add/Remove editing, and atomic rejection; UI tests
cover filtered creation selection, Add source picking and cancellation, pick
order, and unsupported colors.
The eight-case live comparison passed with zero coordinate difference and
matching recorded document state. A [batch-deletion benchmark](../batch-deletion.md)
tracks native conversion and history costs; Rhino performance parity remains unmeasured.

```sh
tools/rhino_oracle/run_headless.sh compare tools/rhino_oracle/fixtures/point_cloud_command.json --timeout 180
```
