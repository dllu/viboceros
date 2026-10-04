# ScalePositions cursor captures

[Command guide](commands/scale-positions.md) · [Oracle](oracle.md)

The capture uses public Rhino commands and SDK snapshots in an empty owned
Rhino 8.32 document on private Xvfb. The input driver targets only the owned
process window, waits for an acknowledged native mouse move at the prescribed
integer pixel, then clicks or types a world point. It never uses the shared
desktop. Front-view rejected clicks are acknowledged by the native mouse-down
callback before Escape cancels the getter.

## Mouse and keyboard input

The [64 cursor recipes](../tools/rhino_oracle/fixtures/scale_positions_cursor.json)
and [raw observations](../tools/rhino_oracle/observations/scale_positions_cursor.json)
cover numeric factors, Enter defaults and references; Top, Front and Perspective
views; near, far and off-axis pixels; Copy; zero and offset origins; and point,
line and rational curve sources. Every recipe seeds preferences through a public
reference command. Cameras, calibrated viewing rays, tight source bounds and
unchanged pending geometry are captured before the final input.

A 1D mouse reference follows the closest point between the viewing ray and the
origin-to-first-reference line. This allows Front input even with an edge-on
WorldXY construction plane. Numeric directions and 2D/3D references instead
intersect the construction plane through the origin. An edge-on free-plane
click leaves the native getter active. Typed world targets use the supplied
coordinates, with projected distances for 1D references.

Numeric 1D input at the world origin completes without an edit or a copy and
retains the previous factor. Reference input at the same origin still works.
Offset-origin numeric and Enter input produce the expected directional
translations for points, lines and rational curves. Factors 2, 3 and the
remembered value 0.5 are checked independently.

## Origin cutoff and retained discrepancy

The [64 origin recipes](../tools/rhino_oracle/fixtures/scale_positions_origin.json)
and [observations](../tools/rhino_oracle/observations/scale_positions_origin.json)
check positive and negative axes, diagonal origins, magnitudes from `1e-10`
to `1e-3`, neighboring doubles around candidate tolerances, and Copy. Macro
numbers use the shared integer-based round-trip formatter; ordinary legacy
Python formatting rounds some of these inputs before producing the token.

The controlled results are consistent with an inclusive cutoff at `2^-23`
for each origin coordinate, apart from the immediate-above diagnostics below. A diagonal with coordinates `1e-7` is also suppressed even though
its Euclidean distance exceeds this cutoff. These tests distinguish the
component test from a radial tolerance. Changed document tolerance has not
been compared.

Inputs at the first double above `2^-23` (`scale-positions-origin-60` and `61`)
remain diagnostics. An [eight-case repeat](../tools/rhino_oracle/observations/scale_positions_boundary_diagnostic.json)
made an in-place edit and saved factor 3; the full origin matrix made no edit
and retained factor 2 for the same prescribed recipe and source. Copy made no
copy in either capture. Both raw outputs are retained without normalization.
These two recipes are excluded from application parity counts. The application
currently uses the inclusive component cutoff for Copy and in-place input.
Behavior immediately outside the boundary remains unresolved.

## Comparison scope

All 64 cursor recipes and 62 origin recipes replay through the command prompt
and the real viewport cursor resolver. They compare complete geometry definitions,
selection, EndCommand,
post-macro state, external Undo/Redo and remembered Mode/Copy/factor. Preview
maps are checked against clicked placements without editing the document or
preferences. Native preview appearance is not captured.

Independent Python equations intersect each recorded viewing ray with the
prescribed plane or reference axis, compute the factor, then translate all
source points or control points using its tight bounding center. Weights,
knots, degree and domain remain unchanged. Geometry comparisons use absolute
`1e-9`; remembered factors printed by Rhino use a separate `5e-6` tolerance
because prompt text is rounded. Enter workflows obtain their own registry
default rather than injecting the observed factor.

The [capture manifest](scale-positions-cursor-provenance.json) records private
schemes, source and artifact hashes, counts and limitations. These bounded
cases do not establish full native parity. Oblique CPlane mouse input, snapping,
reference distance locks, cross-viewport input, native preview appearance and
performance remain unverified. Historical scripted 1D default witnesses still
contain a context-dependent exception; those raw diagnostics remain retained.

```sh
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle rhino tools/rhino_oracle/fixtures/scale_positions_cursor.json --scheme VibocerosOracleScalePositionsCursorFresh --output /tmp/scale-positions-cursor.json --timeout 300
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle rhino tools/rhino_oracle/fixtures/scale_positions_origin.json --scheme VibocerosOracleScalePositionsOriginFresh --output /tmp/scale-positions-origin.json --timeout 300
cargo test -p viboceros --bin viboceros scale_positions
python3 -m unittest tools.rhino_oracle.test_scale_positions_cursor
```
