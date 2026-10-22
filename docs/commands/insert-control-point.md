# InsertControlPoint

Run `InsertControlPoint`, select one curve or surface, and pick locations on it.
Each accepted pick updates the preview and its control markers. Enter commits
all picks as one edit; ordinary Undo restores the source. Esc discards the
preview and retains existing redo history. This command changes the shape.
Typed startup options and viewport picks use this same staged getter; unsupported
options, including `Direction=Both`, fail without starting a different prompt.

For surfaces, `Direction=U|V` names the row orientation: U adds a V-indexed row.
`Toggle` swaps directions. `Midpoint=Yes` places the new control halfway between
the neighboring controls in its Greville interval. These preferences belong to
this command independently of InsertKnot and RemoveControlPoint.

`Extend` switches the next surface pick to an end choice. It adds an outside
control row half an end-polygon interval beyond that end, then returns to
insertion. Rhino rebuilds the entire extended control net with unit weights
and clamped uniform knots in both directions. Consequently, Extend also opens
periodic directions. The surface's UV domains remain unchanged.

The source remains immutable during picking. Commit validates its identity,
geometry snapshot and document tolerance, retains object attributes and groups,
and creates one history item. Composite containers and trimmed B-reps retain
their geometry text. The shared certified surface editor updates trim images
when the underlying surface changes.

One-line scripts retain the point followed by options syntax:

```text
InsertControlPoint 4,3,1 Direction=V Midpoint=Yes
```

For repeated input and option changes, use the sequential form:

```text
InsertControlPoint Points Direction=V Midpoint=Yes 4,3,1 4,4,1
InsertControlPoint Points Direction=U Extend 4,5,1
```

Both forms use `insert_control_point::Draft`. Uniform curve charts keep their
original knot spacing and gain a span, starting at zero. Nonuniform charts
retain their knots and insert the selected parameter. Analytic curves retain
their source chart. Composite edits preserve parent intervals and untouched
leaves; exact junction picks reproduce the recorded raw parent/local chart
lookup and first-control fallback. Surface edits retain their UV domains.
The lower-level `NurbsCurve::try_insert_control_point` remains available for
parameter-preserving insertion.

The Python oracle's `curve_insert_control_point_geometry` and
`surface_insert_control_point_geometry` operations also use this draft. Their
parameter locates a model point on the source; the command then finds its own
closest parameter. Coincident source points can therefore select a different
parameter. Surface protocol directions name the changing parameter axis,
whereas the command names the row orientation. A completed native cancellation
returns the unchanged definition; command failures remain errors. A separate
[44-case protocol validation](../insert-control-point-protocol-validation.json)
checks complete NURBS definitions with both backends at `1e-8`.

Picks at a full-multiplicity knot in a nonuniform chart, including closed
circle/ellipse seams, cancel without committing staged edits. Exact open
surface boundary inputs also cancel. Uniform open curve endpoint picks can
duplicate an endpoint control. Midpoint input can select an adjacent interval
at a nonuniform endpoint.

The [validation receipt](../insert-control-point-validation.json) records 124
actual Rhino 8.32.26160.13001 command cases: 82 insertion workflows, 24 extension
workflows, 15 endpoint/seam/junction decisions, and three analytic-chart checks. API and application tests
compare definitions, sampled geometry, metadata, selection and undo/redo at
`1e-8`, with portable and native backends. The captures include rational and
periodic nets, composite curves, trims, custom domains, transformations,
negative weight gauges, repeated picks, postselection and cancellation.

This Rhino version offers U and V rather than Both, and does not offer SubCrv
or local Undo for this command. The separate
[option rejection capture](../../tools/rhino_oracle/observations/insert_control_point_options.json)
records rejection of Both followed by a valid U pick; it is not an additional
geometry agreement case. Viboceros rejects those options. Polysurfaces
are rejected. General non-clamped inputs, every composite arrangement, native
mouse input, all camera/high-DPI configurations, edited-file roundtrips and
general performance remain unqualified. Native spatial trim-edge definitions
are not captured; their continuity uses the existing document-tolerance
certificate.

Reference: [McNeel command help](https://docs.mcneel.com/rhino/8/help/en-us/commands/insertcontrolpoint.htm).
The recorded command behavior resolves differences from the help page's option
list.
