# Sphere volume selection

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

Points, line segments, circles, polylines, and mesh faces use direct geometric
checks. Other curves use 128 equal-length segments, and surfaces and B-reps
use 16 tessellation samples per span. Near tangencies on those sampled objects
may differ from exact Rhino selection. While picking points, enter a
`SelectionMode` option to change the mode.

This follows Rhino's
[SelVolumeSphere](https://docs.mcneel.com/rhino/8/help/en-us/commands/selection_commands.htm#SelVolumeSphere)
selection modes.
