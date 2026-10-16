# CrvDeviation

Select two curves and run `CrvDeviation`, or run the command first, pick two
curves, and press Enter. The report shows minimum and maximum distances plus
the overlap endpoints' distances, native parameters and world coordinates.
Viewport marks show the minimum in green, maximum in red, and overlap ends in
white in wireframe, shaded, and ghosted views. Enter or **Done** dismisses the
report. Escape, closing the window, or starting another command discards its
transient display.

Set `KeepMarks=Yes` in the report window or command line before finishing with
Enter. This adds colored points and connecting lines on the current layer as
one undoable edit. A pair within model tolerance produces a single point.
Source curves remain intact. Measuring and changing display options preserve
Undo/Redo. Source geometry or tolerance changes invalidate the report so its
marks cannot be committed against changed sources.

```text
CrvDeviation
KeepMarks=Yes
<Enter>

CrvDeviation End
CrvDeviation Cancel
```

At the curve picker, type `SubCrv`, select a curve, then pick the portion's start
and directed end. You can also start with `CrvDeviation SubCrv`. Repeat for a
second portion or select a whole curve, then press Enter to measure. The shared
picker supports preview, numeric arc lengths with confirmation, and direction
locking. Empty Enter abandons the current portion and returns to curve picking;
Escape cancels, and `SelNone` clears the collected inputs.

Temporary portions replace whole-object selection of their owner. Two portions
from one owner are allowed, and exactly two effective inputs are required.
Sources are never shortened in the document. `KeepMarks` still creates just
the measurement marks as one undoable edit. Source snapshots invalidate the
analysis if the original geometry changes.

For complete scripted invocations, use one or two `SubCrv=id,start,end` arguments,
with endpoints in the curve's native parameter domain. Other selected curves
supply whole inputs. Inputs follow document object order; portions from the
same owner retain argument order. Reported parameters belong to each measured
piece: reverse-open pieces have reversed, negated domains, and closed portions
can cross the existing seam into an extended domain.

```text
CrvDeviation SubCrv=1,3,7 SubCrv=2,3,7 KeepMarks=Yes
CrvDeviation End
```

See the [temporary-input validation](../curve-deviation-subcurves-validation.json)
and [native command comparison](../../tools/rhino_oracle/observations/curve_deviation_subcurves_comparison_20261016.json).

Line pairs work in the portable build. They use an analytic correspondence in
planes perpendicular to the aligned direction bisector, with exact rational
clipping and minimization after computing that direction. This reproduces the
original parallel, partial, reversed, and centered angled fixtures: the maximum for
crossing finite lines can exceed their nearest-point distance. Perpendicular
lines and intervals touching at only one projected position have no report.
Native parameter domains are preserved. Directions within 64 machine epsilons
of perpendicular are treated as ambiguous. The broader asymmetric and nearby
endpoint captures expose additional line interval and distance differences;
the report therefore identifies line correspondence as approximate too.

Curved inputs require `native-smlib`. They reuse the usd-brep/SMLib
[distance backend](../curve-distance-queries.md) with projected endpoint ranges.
Corresponding natural ends perpendicular to the aligned endpoint chords are
preserved within model tolerance, so translated profiles retain their ends.
Witness parameters use the existing representation map, preserving
circular seam visits and composite arc branches without closest-point recovery.
For circle pairs on parallel planes, an angular overlap requires both projected
centers to be strictly inside the other circle. General curved correspondence
is still approximate and the report states that limitation. In particular:

- Captured offset cubic and arc minima differ from Rhino's reported values.
- Projected endpoints do not reproduce all native interval boundaries or
  mixed open/closed correspondences.
- Tied extrema can choose different points and parameters, including different
  permanent marks even when the minimum/maximum distances agree.

The retained 22-case workflow capture includes circle overlap boundaries,
crossing lines from 5 to 175 degrees, and five permanent-mark cases. Its
[comparison](../../tools/rhino_oracle/observations/curve_deviation_workflows_comparison_20261016.json)
keeps SDK scalar values, printed native command values, and unordered marker
geometry separate. All 22 report decisions and 16 maximum SDK distances agree
within `1e-6`. Fifteen of 16 minima agree with the printed command output at its
observed six-significant-digit precision plus `1e-6`; only two of five permanent
marker sets match. Rhino's SDK and command output themselves have different
minimum values in several crossing cases. No complete command, continuous
global-extremum, or performance parity is claimed. These numbers describe the
initial committed workflow capture at its recorded revision.

The [broader overlap study](../curve-deviation-overlap.md) retains 52 additional
native cases, including asymmetric lines and near-endpoint transitions. On its
25-case main corpus, preserving transverse natural ends improves matching
interval-end marker sets from 9 to 16. The remaining interval, minimum and
maximum differences remain explicit failing comparisons.

To reproduce the command-side local observations:

```sh
cargo run --quiet -p viboceros-oracle --features native-smlib -- \
  tools/rhino_oracle/fixtures/curve_deviation_workflows.json /tmp/crv-deviation.json
python3 -m tools.rhino_oracle.curve_deviation_comparison \
  tools/rhino_oracle/fixtures/curve_deviation_workflows.json /tmp/crv-deviation.json \
  tools/rhino_oracle/observations/curve_deviation_workflows_rhino_20261016.json \
  /tmp/crv-deviation-comparison.json
```

For `curve_deviation` oracle recipes, an explicit `keep_marks` boolean selects
actual local command execution followed by `End`. Omitting it retains the
earlier backend-only diagnostic. Native captures always use an owned private
headless instance. See the [validation record](../crv-deviation-validation.json).
Recipes with `subcurves` use normalized endpoint fractions and require explicit
`postselect=true` and `keep_marks`. Their native SDK capture measures whole
sources, while the local SDK-shaped values measure the selected inputs. The
comparison therefore uses actual native command history and marks for those
recipes and keeps the SDK scopes separate.
