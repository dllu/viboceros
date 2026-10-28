# Sweep1 validation and limits

[Command guide](../sweep1.md) · [Construction](construction.md)

## Current qualification

[Automatic placement](../sweep1-placement.md) covers rail/profile selection,
minimum-distance placement, direction and interval policy. The subsequent
[arc parameter checks](../sweep1-arc-parameters.md) qualify 36 independent command
outputs plus the original thirty-degree regression. Both complete rational
boundary atlases participate; matching positive V weight rows extend the bounds
to all surface points. The recorded maximum bound is below `8.59e-7`, against
comparison epsilon `1e-6` and construction tolerance `1e-7`. This permits different
parameter clocks and face partitions; it does not prove matching derivatives or
topology. See the [validation record](../sweep1-arc-parameters-validation.json)
for backend results and source inventories.

A separate [distance certificate](../surface-distance-bounds.md) proves that the
captured reversed spatial case still has an image gap above `2.376e-6`. That is a
construction difference. A close witness or successful closest-point search on
another case does not establish complete surface agreement. Full Sweep1
compatibility remains unfinished.
The separate [Global blending comparison](global-blending.md) now retains 18
complete definitions, passing controls, and certified multi-profile gap witnesses.

## Native regressions

Independent native tests cover straight local/global blends, one- and two-profile
analytic circular sweeps, off-grid fitting errors below `1e-9`, exact supplied
rational sections, multi-profile knot placement, Roadlike orientation, unrefitted rail preservation and
Greville interpolation, common rational rail-weight signs/scales `±1e±280`, relative
profile-weight scales, Euclidean versus homogeneous blending, positive signed
weight functions and both crossing/tangent poles, invalid
inputs, and forward/inverse arc-length queries. Command tests check multi-profile
blending and both frame styles, closed and creased profile topology, retained
document state, and undo. A failing-then-fixed closest-point regression checks a skew bilinear
surface: clamping a coupled Newton step could stall before the minimum along
an edge. All four natural boundary curves are now searched independently in
addition to the bounded multi-start interior search. This does not certify a
global minimum on arbitrary surfaces. Tight arc/spatial curve-fit regressions
sample independent off-grid points, and a nonuniform/kinked cubic collocation
test compares the banded solve against independent dense elimination.
Curvature-aware curve closest-point refinement also fixes the five-profile
boundary search. Independent polynomial stationarity gives its unique local
minimum at U `0.00024483021033695`; the former tangent-only search stopped near
`0.00002386546448`. Oracle precision tests request and check a geometric tolerance
rather than requiring an iterative result to equal a special-case seed bit-for-bit.

## Historical sampled comparisons

The following records describe the earlier fixed-basis implementation. Their
measurements are retained as historical evidence; they are not a rerun of every
fixture against the current revision. Later complete-image and certified-distance
results are described above.

Reference construction uses public
[RhinoCommon CreateFromSweep](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/M_Rhino_Geometry_Brep_CreateFromSweep_5.htm)
(the 13-argument, since-Rhino-7 overload available in Rhino 8), and actual
[`_-Sweep1` commands](https://docs.mcneel.com/rhino/8/help/en-us/commands/sweep1.htm)
with `RefitRail=No` and `Yes`. That API overload uses the refitted path; it is
not a stand-in for the default unrefitted command. The fixture's `refit_rail`
flag applies only when `command=true`. Roadlike is checked through the API,
not the actual-command adapter. The script macro explicitly sets
`_Style=_Freeform`, `_Simplify=_None`, `_Closed=_No`,
`_ShapeBlending=_Local|_Global`, and `_RefitRail=_No|_Yes`.
Earlier instrumentation incorrectly used dialog property names; Rhino could
ignore them while `RunScript` returned success. The corrected adapter checks
new history for rejected options. All permanent command cases below were
rerun with it. Rhino runs use an owned private Xvfb display;
no proprietary code was inspected.

Each of the 62 cases compares 135 unrounded closest-point results: 81 original
Rhino surface-grid points plus 54 world-axis offsets from corners, edge
midpoints, and the center. These sample geometry and extents but are not a
bidirectional continuous distance certificate. Command probes compare output
surface geometry only, not Rhino selection/attributes, and report zero timing
to indicate untimed command execution.

| Fixture | Cases | Recorded result at absolute `1e-6`, relative `1e-12` |
| --- | ---: | --- |
| `sweep1.json` | 11 | Pass; maximum coordinate difference `2.91e-7`. |
| `sweep1_command.json` | 6 | Pass; maximum `2.32e-7`, including both refit choices. |
| `sweep1_multisection.json` | 13 | Pass; maximum `1.35e-7`, including Local three/four/five profiles, close stations, retained spatial/rational/degree-five/arc bases, and two-profile Global commands. |
| `sweep1_weights.json` | 19 | Pass; maximum `8.01e-7`, including mixed degrees, multi-span profiles, unequal weights, retained signed-weight surfaces, varying control positions, and both blend/refit options. |
| `sweep1_curved_blend.json` | 2 | Fail: two-profile arc `1.05e-6`, spatial rail `1.28e-6`. |
| `sweep1_diagnostics.json` | 2 | Fail: rational closest-point results `0.0457`, three-profile Global `0.198`. |
| `sweep1_basis_diagnostics.json` | 5 | Fail: Global commands, refit degree/parameter differences, and a nonminimal Rhino boundary answer. |
| `sweep1_weights_diagnostics.json` | 4 | Fail: two-profile Local curved refits, the refit of a degree-five rail, and another nonminimal Rhino rational boundary answer. |
| `sweep1_closed_circle_world.json` | 1 | 81 on-surface world queries agree within `4.3e-11`; nine off-surface Rhino closest-point results differ by up to `4.2e-6`. |
| `sweep1_closed_circle_refit.json` | 1 | `RefitRail=Yes`; all 81 world queries match within `4.3e-11`. |

The [closed-circle command input](../../tools/rhino_oracle/fixtures/sweep1_closed_circle_world.json)
and [Rhino observation](../../tools/rhino_oracle/observations/sweep1_closed_circle_world.json)
retain 90 world-space closest-point queries. The first 81 lie on the intended
cylinder and have maximum native/Rhino coordinate difference `4.3e-11`.
For the other nine, the native exact cylinder returns the analytic nearest
point, while Rhino's closest-point search deviates by up to `4.2e-6`.
The separate [definition input](../../tools/rhino_oracle/fixtures/sweep1_closed_circle_definition.json)
and [observation](../../tools/rhino_oracle/observations/sweep1_closed_circle_definition.json)
show Rhino's dense cubic rail direction (131 controls); native retains the
exact rational circle basis (9 controls). Equal UV fractions therefore sample
different points despite matching geometry. A closed circular profile also
produces a native solid, but its 81-query Rhino probe timed out before returning
an observation.

The [refitted closed-circle input](../../tools/rhino_oracle/fixtures/sweep1_closed_circle_refit.json)
and [Rhino observation](../../tools/rhino_oracle/observations/sweep1_closed_circle_refit.json)
retain 81 matching world queries. Separately, at the default document
tolerance the native nonrational cubic fit uses 515 controls and preserves the
surface seam. The previous 512-control ceiling stopped three controls short.
The cubic fit now accepts radius-10 and radius-100 circles at the same absolute
tolerance under a 4,096-control budget. More demanding curves can still exhaust
this limit; the surface also has a separate 262,144-control budget.

Construction tolerance in these fixtures is `1e-7`; passing the looser `1e-6`
comparison is not evidence of agreement at construction tolerance. Independent
SciPy B-spline evaluation and bounded least-squares searches reproduced the
two curved-blend geometry differences; search errors were below `2e-8`.
Those cases are retained as failing diagnostics, not hidden by increasing
the comparison epsilon. The native fitter uses integrated arc length; probes
of a quarter-circle and a spatial cubic found Rhino refit domain lengths equal
to 256 uniform rational-parameter chords. This observation is not a complete
model of Rhino's refitter or its station inversion.

The refitted unequal-weight fixture's control points and knots now agree with
Rhino within `4e-15`, after one geometry-preserving common weight scale. Its two
remaining closest-point differences are nonminimal Rhino answers: for query
`(0,-0.2,2.5462962962962963)`, the native distance is `0.2`, versus Rhino's
`0.2051579914`. Positive weights and nonnegative control Y certify the `0.2`
lower bound; a native regression attains it on both boundary witnesses. This
does not certify general surface closest-point searches. The mixed-degree
refitted weight fixture retains another instance of this Rhino boundary-search
discrepancy. The refitted degree-five rail retains a separate knot-basis
construction difference (`0.00141`), not a weight-policy error. Varying-profile
refits pass the `1e-6` geometry comparison despite differing control coefficients;
equal coefficient counts alone are not proof of equal geometry.

```sh
tools/rhino_oracle/run_headless.sh compare tools/rhino_oracle/fixtures/sweep1.json \
  --absolute-epsilon 1e-6 --relative-epsilon 1e-12
tools/rhino_oracle/run_headless.sh compare tools/rhino_oracle/fixtures/sweep1_command.json \
  --absolute-epsilon 1e-6 --relative-epsilon 1e-12
tools/rhino_oracle/run_headless.sh compare tools/rhino_oracle/fixtures/sweep1_multisection.json \
  --absolute-epsilon 1e-6 --relative-epsilon 1e-12
tools/rhino_oracle/run_headless.sh compare tools/rhino_oracle/fixtures/sweep1_weights.json \
  --absolute-epsilon 1e-6 --relative-epsilon 1e-12
```

## Remaining work and performance

General closed-rail holonomy correction and cyclic profile blending, rail miters,
complete unrefitted multi-profile compatibility,
automatic branch/seam alignment and remaining
Rhino section-refit options are not implemented. Section rebuilding now has
[separate complete-image checks](rebuilding.md). These are explicit limits,
not evidence that the overall project goal is complete. Performance is also
unfinished: short release timings for the spatial refitter are tracked separately
from geometry comparisons; dense-to-banded solving alone did not remove the
cost of repeated accurate arc-length inversion.

Cross-section Refit remains in development. The native oracle accepts
`refit_tolerance` independently of rail refitting and rejects simultaneous
section rebuilding. Frozen `sweep1_refit_*` captures contain 70 sweep cases and
16 public `Curve.Fit` diagnostics, with archived producers. Identical translated
profile pairs use a different fitting policy from one profile; `Curve.Fit`
also produces a different result. Complete geometry comparison for Viboceros
section refitting remains unfinished.
The exact Bernstein certificate in `test_sweep1_refit.py` also proves that the
`0.001` native setting leaves a source point more than `0.0015` from the output
profile boundary. The setting and a certified continuous deviation bound need
separate semantics; agreement with Rhino still requires the geometry comparison.

An exact-key, bounded sample cache reduced single-profile spatial timings from
about 43 ms to 25 ms, versus 15 ms Freeform and 12 ms Roadlike in Rhino under
FEX/Wine. Two-profile spatial construction measured 26 ms versus 19 ms.
Most simple cases were faster natively, but these separate short runs are not controlled benchmarks
and do not satisfy the project's general performance requirement.
