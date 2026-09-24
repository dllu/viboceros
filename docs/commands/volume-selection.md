# Box and sphere volume selection

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

The sphere checks points, line segments, circles, polylines, and mesh faces
directly. The box samples NURBS curves and polycurves; the sphere also samples
arcs and ellipses. These curves use 128 equal-length segments, while surfaces
and B-reps use 16 tessellation samples per span. Near tangencies on sampled
objects may differ from exact Rhino selection. While picking points, enter a
`SelectionMode` option to change the mode.

This follows Rhino's
[SelVolumeSphere](https://docs.mcneel.com/rhino/8/help/en-us/commands/selection_commands.htm#SelVolumeSphere)
selection modes.
