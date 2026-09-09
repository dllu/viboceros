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

## Limits

Per-point colors and Add/Remove editing are not implemented. `UsePointColors=Yes`
and Add/Remove arguments return errors without mutation. A typed invocation
with a preselected cloud also reports unsupported editing. Use `Explode` to
extract individual points from a cloud, or `ExtractPt Output=PointCloud` for
supported geometry extraction workflows. `PointGrid` creates rectangular clouds.

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
undo/redo, group restoration, and atomic rejection; UI tests cover filtered
selection, pick order, cancellation, and unsupported colors.
The eight-case live comparison passed with zero coordinate difference and
matching recorded document state. A [batch-deletion benchmark](../batch-deletion.md)
tracks native conversion and history costs; Rhino performance parity remains unmeasured.

```sh
tools/rhino_oracle/run_headless.sh compare tools/rhino_oracle/fixtures/point_cloud_command.json --timeout 180
```
