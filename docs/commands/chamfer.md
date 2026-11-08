# Chamfer

[Command reference](README.md) · [Qualification](../chamfer-getter.md)

Start `Chamfer`, pick the first curve on the side to retain, then pick the second.
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
A setback can cross an extension into the original curve. Zero-distance trimming
records replacements even when geometry is identical, clearing pending Redo.
Without trimming, zero distances make no edit after a contact is found.
Join=Yes keeps the sharp joined corner.

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

With distinct closed sources, each closed parent stays whole. Trim changes
only an open source. Join reuses the first source ID when it is open; otherwise
it creates an ungrouped result with that source's attributes. Intersections,
overlap boundaries and the picks choose each setback direction. Straight open
sources can extend to a closed parent. Open arcs and NURBS also support
[nonmeeting continuation](../chamfer-extension.md). Nonmeeting arcs against a
closed source require `ExtendOtherCurvesBy=Line` when `ExtendArcsBy=Line`;
the mixed Smooth setting is rejected without edits or Redo loss.
See [closed-source qualification](../chamfer-closed.md).

Nonmeeting curved sources consider both ends and choose the least added
continuation. Closed-source intersection choices and setback directions follow
the native parameter chart, so moving a closed seam can affect the result.
See [end and seam selection](../chamfer-branch.md).

At complete consumption, separate outputs keep the original piece; joining omits
the consumed piece. Oversized lines and tangent NURBS continuations can produce
separate retained pieces that do not meet the bevel, matching Native behavior.
Circular setbacks can wrap into a major arc. Invalid disconnected joins are
rejected without edits or Redo loss. See [open-curve contact and consumption
coverage](../chamfer-open.md). Arbitrary folded curves and all intersection
branches remain unqualified.
