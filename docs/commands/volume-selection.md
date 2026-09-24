# Box, sphere, pipe, and object volume selection

`SelBox [base-corner opposite-base-corner height] [SelectionMode=Window|Crossing|InvertWindow|InvertCrossing]`
selects against a three dimensional box aligned to the active construction
plane. Enter it without coordinates to pick the two opposite base corners and a
height point. Typed height may be a signed distance or a point. For example:

```text
SelBox 0,0,0 5,4,0 3 SelectionMode=Crossing
```

The box checks points and line segments directly, clips mesh triangles against
the six box planes, and checks circles, circular arcs, and ellipses against
those planes using their analytic parameterization. Rhino's
[SelBox](https://docs.mcneel.com/rhino/8/help/en-us/commands/selection_commands.htm#SelBox)
also uses point samples and warns that it may miss some crossing objects.

## Pipe

`SelVolumePipe [curve-id] radius [SelectionMode=Window|Crossing|InvertWindow|InvertCrossing]`
selects against a constant-radius tube around a curve. Select one curve first
and enter a positive radius, or pass its object ID explicitly. Enter the
command without a radius to pick a centerline curve and then a radius point
in the viewport. For example:

```text
SelVolumePipe 0.5 SelectionMode=Crossing
```

The tube includes points within the radius of its centerline, including round
ends on an open curve. The source curve is excluded from the result. Point
objects use the core curve closest-point query. Line and polyline centerlines
are checked segment by segment; other centerlines use 128 equal-length samples
for crossing tests. Mesh faces are checked for segment passage through their
interiors as well as near edges and vertices. Curved target objects and
nonstraight tube Window tests use samples, so thin boundary cases may differ
from Rhino. The command supports the four [Rhino selection modes](https://docs.mcneel.com/rhino/8/help/en-us/commands/selection_commands.htm#SelVolumePipe).

## Object

`SelVolumeObject [object-id] [SelectionMode=Window|Crossing|InvertWindow|InvertCrossing]`
selects against a closed mesh, B-rep, or closed NURBS surface. Select one
source object first, pass its ID, or enter the command without a source to pick
one in a viewport. The source object is excluded from the result. For example:

```text
SelVolumeObject SelectionMode=Crossing
```

The mesh source must have a closed, manifold, consistently oriented shell.
B-reps and NURBS surfaces are tessellated before classification. Point queries
distinguish the interior, boundary, and exterior. Repeated point queries use a
face bounds tree built once for the source mesh. Line crossings use the same
tree to narrow candidate faces and select lines even when both endpoints are
outside. Nonplanar mesh quads are checked as two triangles. Curved targets and
Window tests on nonconvex solids use samples, so thin boundary cases may differ
from Rhino's
[SelVolumeObject](https://docs.mcneel.com/rhino/8/help/en-us/commands/selection_commands.htm#SelVolumeObject).

## Sphere

`SelVolumeSphere [center radius] [SelectionMode=Window|Crossing|InvertWindow|InvertCrossing]`
selects objects against a sphere in model coordinates. Enter the command without
coordinates to pick a center and radius point in a viewport. The center accepts
`x,y,z`, `x,y`, or three space-separated numbers. Radius must be positive and
finite. Crossing is the default. For example:

```text
SelVolumeSphere 0,0,0 5 SelectionMode=InvertCrossing
```

Window selects objects wholly inside the sphere. Crossing includes partial
intersections. InvertWindow selects objects wholly outside; InvertCrossing
also includes partial intersections. Hidden and locked objects are skipped,
and the command does not create geometry or change undo history.

The sphere checks points, line segments, circles, circular arcs, ellipses,
polylines, and mesh faces directly. For arcs, it checks the nearest and farthest
points within the sweep and both endpoints. Ellipses use nearest and farthest
point queries. The box samples NURBS curves and polycurves; the sphere also
samples those curves. These curves use 128 equal-length segments, while
surfaces and B-reps use 16 tessellation samples per span. Near tangencies on
sampled objects may differ from exact Rhino selection. While picking points,
enter a `SelectionMode` option to change the mode.

This follows Rhino's
[SelVolumeSphere](https://docs.mcneel.com/rhino/8/help/en-us/commands/selection_commands.htm#SelVolumeSphere)
selection modes.

The [sphere selection oracle fixture](../../tools/rhino_oracle/fixtures/volume_selection.json)
contains nine shared curve cases, including all four modes and narrow arc and
ellipse intersections. The native probe runs these through the actual command. A live
Rhino observation has not been saved: the current direct ARM64 Wine launch
exits in `.NET` initialization before the Python worker starts.
When Rhino starts, run `python3 -m tools.rhino_oracle compare
tools/rhino_oracle/fixtures/volume_selection.json --timeout 360` from the
repository root to compare actual command selections.
