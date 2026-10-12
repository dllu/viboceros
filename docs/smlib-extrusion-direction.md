# Numeric curve extrusion direction

Numeric `ExtrudeCrv` now chooses one direction for the selected curve batch.
Planar parallel profiles, including profiles on different parallel planes, use
their common normal. Its largest world-axis component is oriented positive,
independent of curve reversal and construction-plane orientation. A line can
share a nonlinear profile's plane direction. Incompatible/nonplanar collections
and a single unconstrained line use the active CPlane Z direction. Parallel lines
use their containing plane when their positions resolve one. Signed
distance and BothSides are applied afterward. Explicit base/target points retain
their world-space vector.

Native builds import each curve once, translate its control directions to a
common origin, and use usd-brep's existing `SmPointSet3d` plane classifier. This
removes offset differences while retaining each curve's directional constraints.
No new plane fitter or extrusion builder is added. The native handles serve
both classification and the existing sheet/solid producers. BothSides placement
uses the existing owned-geometry affine transform. Portable builds reuse the
geometry crate's existing control-polygon plane classifier through a shared
normal-returning helper.

The adapter rechecks the native point-set classification after requesting the
normal because that query can rerun the fitter and replace the cached type.
See the [source audit](usd-brep-limitations.md).

Output geometry is staged before document edits. Current-layer attributes,
source groups, selection, DeleteInput and one-step Undo/Redo follow the existing
flow. The command now receives the active `CommandContext` for CPlane fallback.
Zero-distance snapping, SubD output, ToBoundary and complete interactive preview
semantics remain further work. The
[official command documentation](https://docs.mcneel.com/rhino/8/help/en-us/commands/extrudecrv.htm)
describes plane-normal defaults and CPlane fallback.

The direction fixture covers axis-aligned and tilted planes, reversed profiles,
negative distance, BothSides, nonplanar sources, mixed selections and rotated
CPlanes. The oracle records output objects in creation-serial order; its earlier
unsorted object-table iteration reversed multi-source results. Geometry results
retain complete raw NURBS data without comparison-time normalization.
The [retained comparison](smlib-extrusion-direction-comparison.json) records
all 27 matches and also replays the previously mismatched short-span extrusion.
Across six fixtures it records 46 raw matches out of 48, with no execution
failures. The remaining differences are the existing unclamped-path definition
and short-span revolution parameter domain.
These are bounded fixture checks, not full interactive preview or arbitrary-input
parity. Exactly parallel line extrusions are skipped, and DeleteInput removes
only profiles that produced geometry.

The full native command suite passed 1,313 tests (two ignored), and Python oracle
tests passed 1,095 tests. The full native oracle library run passed 475 tests and
failed 20 existing Join expectations, including the audit that expects rejection
of a transitive gap cluster. Those failures cover tolerances, raw geometry and
import restrictions already tracked in [native Join qualification](smlib-joining.md);
the two new extrusion replay tests passed. The full native oracle suite is not
green, and these failures remain visible.
The final focused native checks passed all 70 bridge tests (one ignored),
11 extrusion command tests and both new oracle replay tests. Native workspace
and portable workspace clippy passed with warnings denied. Portable checks
passed nine extrusion command tests, 35 selected curve geometry tests, all 494
oracle library tests and 25 integration tests, including the Join expectations
that fail with the native backend. Workspace formatting also passed.

```sh
CMAKE_BUILD_PARALLEL_LEVEL=10 cargo test --release -p viboceros-command \
  --features native-smlib extrude_curve
CMAKE_BUILD_PARALLEL_LEVEL=10 cargo test --release -p viboceros-smlib \
  --features native --test curve_planes
CMAKE_BUILD_PARALLEL_LEVEL=10 cargo test --release -p viboceros-oracle \
  --features native-smlib curve_extrusion_replay_tests
cargo test --release -p viboceros-oracle curve_extrusion_replay_tests
```
