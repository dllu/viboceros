# Chamfer

[Command reference](README.md) · [Qualification](../chamfer-getter.md)

Start `Chamfer`, pick the first curve near the desired end, then pick the second.
The second pick commits immediately. `Distances` asks for the first distance,
then the second; Enter at the second getter uses the first distance. A number at
curve selection starts the same sequence. Values accept units and arithmetic,
such as `0.02in` and `1/2`.

Two points measure the first distance. The next point measures the second from
the same starting point. If the first distance was numeric, two points measure
the second. Accepted values persist across cancellation, including a first
distance accepted before cancelling the second getter. Escape cancels without
editing geometry or consuming pending Redo.

Distances are straight-line distances from the intersection to each cut point.
`Trim=Yes` is the default. `Join=No` retains each source ID, attributes and groups,
and adds an unnamed, ungrouped bevel on the current layer. `Join=Yes` replaces
the first source in place with the joined curve and removes the second source.
`Trim=No` retains both originals and adds only the bevel; Join is unavailable.
Results are deselected. Undo/Redo and 3DM preserve geometry and metadata.

`ExtendArcsBy=Arc|Line` selects circular or tangent continuation.
`ExtendOtherCurvesBy=Line|Smooth` selects tangent or smooth NURBS continuation.
Defaults are `Arc` and `Line`. Bare extension, Join and Trim options toggle their
values. Retained pieces preserve native arc/NURBS geometry and parameter charts.
A setback can cross an extension into the original curve. Zero distances with
Join=No can finish without an edit; Join=Yes keeps the sharp joined corner.

Headless calls support two selected curves or explicit ordered sources:

```text
Chamfer Distances=0.5,1 Join=Yes
Chamfer Sources=first-id,second-id Pick1=3,0,0 Pick2=4,1,0 Distances=0.5,1
```

`Chamfer 0.5 1` remains a batch shorthand. Without Pick1/Pick2, batch calls use
the nearest endpoint pair. The GUI also accepts options on the starting line.
Native builds delegate NURBS extension and extraction to
[usd-brep](../smlib-curve-connections.md).

Pick two locations on one polyline, polycurve or kinked NURBS to chamfer a
junction. Neighboring smooth pieces identify the junction; flat subdivisions
inside the original interval belong to one picked piece. The original closed
seam remains a picking boundary. Setbacks can consume earlier spans. Closed
parents retain their cyclic remainder; reversing the picks swaps which distance
applies to each side. See [single-parent qualification](../chamfer-self.md).

On an open parent, zero distances still split the source; Join produces a
polycurve through the unchanged corner. Zero distances on a closed parent fail
without an edit. One selected parent with both Pick1 and Pick2, or repeated
explicit source IDs, supports headless calls.

Distinct closed sources are unsupported. Distances consuming a complete
source are rejected atomically. One captured oversized native success produces
nonmeeting retained/bevel endpoints; it is documented as a compatibility gap.
Arbitrary folded curves and all intersection branches remain unqualified.
