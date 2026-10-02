# Remove interior B-rep holes

`Brep::try_remove_holes(&[(face, loop)], tolerance)` removes selected interior
loops. Indices are local to each face. It follows their shared edges, deletes
joined hole wall faces, and removes matching openings on neighboring faces.
Selecting either cap opening of a closed tube produces the same closed outer
cylinder. Traversal stays within each connected component.

The result retains exact surviving surface control nets, knots, domains,
orientation, edge curves, UV trims, and tolerances. Unused vertices and edges
are compacted in source order; boundary, mated, and seam trim classes are updated
from retained edge uses. The complete output is validated before return. Inputs
are immutable. Invalid indices return an error, repeated indices select once,
and a selection containing only outer loops or no loops returns `None`.

`try_remove_holes_with_topology` also returns original wall-face indices and
removed openings on surviving faces, each in source table order. Commands use
this validated traversal result to retain exact wall geometry and naked-hole
curves without inferring topology from compacted output tables.

`Brep::try_remove_all_holes(tolerance)` selects every inner loop. Unlike the
selected-loop overload, it returns an unchanged independent copy when the
source has no holes. These behaviors match the public RhinoCommon
[Brep.RemoveHoles overloads](https://developer.rhino3d.com/api/rhinocommon/rhino.geometry.brep/removeholes).

The [source recipes](../../tools/rhino_oracle/fixtures/brep_remove_holes.json)
and [Rhino 8.32 captures](../../tools/rhino_oracle/observations/brep_remove_holes.json)
contain 28 API cases: single and multiple planar holes, rational annuli,
paraboloid trims, reversed faces, shifted UV domains, closed and open joined
tubes, disconnected caps, no-hole surfaces, empty selections, duplicates, and
outer-loop selections. Both engines use identical source 3dm files exported
from independent input recipes. Full topology, geometry definitions, and
tolerances are compared with absolute epsilon `1e-9` and relative epsilon zero.
No output definitions are used to construct native inputs.

```sh
cargo test --release -p viboceros-geometry brep::untrim
cargo test --release -p viboceros-oracle remove_holes
python3 -m unittest tools.rhino_oracle.test_remove_holes
python3 -m tools.rhino_oracle replay tools/rhino_oracle/fixtures/brep_remove_holes.json --observations tools/rhino_oracle/observations/brep_remove_holes.json --absolute-epsilon 1e-9 --relative-epsilon 0 --timeout 300
tools/rhino_oracle/run_headless.sh compare tools/rhino_oracle/fixtures/brep_remove_holes.json --absolute-epsilon 1e-9 --relative-epsilon 0 --timeout 300
```

These are geometry API comparisons. [UntrimHoles](../commands/untrim-holes.md)
supports typed and viewport component edits, retained trim objects, and grouped
history. Component preselection and window selection remain pending. Harness
timings are untimed and make no speed claim.

## Native command evidence

The [44 component recipes](../../tools/rhino_oracle/fixtures/untrim_holes_components.json)
and [native command capture](../../tools/rhino_oracle/observations/untrim_holes_components.json)
record actual `UntrimHoles` executions with face/edge preselection and owned
mouse input. Typed component edits match their complete geometry and metadata:

- `All=Yes` picks individual faces; whole-object `SelID` picks are ignored.
- Preselected components are edited before the first prompt, using remembered
  options. Picks are applied immediately; Esc leaves completed edits in place.
- Zero clears the length filter. The eight-unit closed hole edge is rejected
  at `7.9` and admitted at `8.0` in both selection modes.
- `KeepTrimObjects=Yes` creates curves for naked holes and retains detached
  B-rep wall geometry for joined holes, with current-layer defaults and no
  source name, color source, or groups.

The [42 split-edge recipes](../../tools/rhino_oracle/fixtures/untrim_holes_limits.json)
and [native capture](../../tools/rhino_oracle/observations/untrim_holes_limits.json)
show that the filter uses the whole hole perimeter, including all edges.
An eight-unit square is rejected at `7.999` and admitted at `8.0`; a ten-unit
rectangle is rejected at `9.999` and admitted at `10.0`, even when its picked
edge is only one unit long. Retained curves preserve trim order and source
parameter intervals. Split UV trims are classified from their complete control
nets so the shared 3dm export preserves isoparametric flags exactly.

The helper uses a shaded viewport for interior face mouse picks and requests
each real click only after its corresponding command prompt. A stalled pick
cancels only the worker's owned private window.

The [eight Undo recipes](../../tools/rhino_oracle/fixtures/untrim_holes_undo.json)
and [native capture](../../tools/rhino_oracle/observations/untrim_holes_undo.json)
also verify restoring the last edit, deleting its retained objects, and repicking
before Enter or Esc. The [16 history recipes](../../tools/rhino_oracle/fixtures/untrim_holes_history.json)
and [native capture](../../tools/rhino_oracle/observations/untrim_holes_history.json)
verify that multiple picks share one external Undo/Redo record, including when
Esc finishes or local Undo discards the second pick. Multiple distinct
preselected components return Failure without edits; duplicate references to a
single component are accepted once. The viewport uses the same prepared edit
and incremental history API. Component preselection, window selection, and
native option changes between picks remain pending. Reproduce the captures with
independently exported owned sources:

```sh
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.untrim_holes_capture tools/rhino_oracle/fixtures/untrim_holes_components.json --timeout 300
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.untrim_holes_capture tools/rhino_oracle/fixtures/untrim_holes_history.json --timeout 300
```
