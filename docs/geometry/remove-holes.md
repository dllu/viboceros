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

These are geometry API comparisons. The interactive `UntrimHoles` command,
its face/edge selection, maximum-edge-length filter, retained trim objects,
and command history remain under implementation. The command is not registered
as a completed feature. Harness timings are untimed and make no speed claim.

## Native command evidence

The [44 component recipes](../../tools/rhino_oracle/fixtures/untrim_holes_components.json)
and [native command capture](../../tools/rhino_oracle/observations/untrim_holes_components.json)
record actual `UntrimHoles` executions with face/edge preselection and owned
mouse input. They establish the next implementation requirements:

- `All=Yes` picks individual faces; whole-object `SelID` picks are ignored.
- Preselected components are edited before the first prompt, using remembered
  options. Picks are applied immediately; Esc leaves completed edits in place.
- Zero clears the length filter. The eight-unit closed hole edge is rejected
  at `7.9` and admitted at `8.0` in both selection modes.
- `KeepTrimObjects=Yes` creates curves for naked holes and retains detached
  B-rep wall geometry for joined holes, with current-layer defaults and no
  source name, color source, or groups.

The helper uses a shaded viewport for interior face mouse picks and requests
each real click only after its corresponding command prompt. A stalled pick
cancels only the worker's owned private window. These records do not prove the
pending Viboceros command, internal Undo, window selection, or option changes
between picks. Reproduce them with independently exported owned sources:

```sh
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.untrim_holes_capture tools/rhino_oracle/fixtures/untrim_holes_components.json --timeout 300
```
