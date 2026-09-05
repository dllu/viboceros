# One-rail sweeps

[Architecture](architecture.md) · [Surface commands](commands/surfaces.md) · [Oracle](oracle.md)

`Sweep1` has an independent geometry model, rail-basis construction,
and a command adapter. This is a partial implementation, not complete Rhino
sweep compatibility.

## Command workflow

```text
Line 0,0,0 0,0,5
SelLast
SetObjectName Rail
Line 0,0,0 2,0,0
SelLast
Sweep1 RailName=Rail Parameters=0
```

`RailName` resolves one unique curve name case-insensitively; the rail need not
be selected. Without it, the last selected curve is the rail. Other selected
curves are profiles in the document's selection order. `Parameters=` is required:
supply one finite, strictly increasing **native rail parameter** per profile,
not normalized fractions or distances unless that is the rail's actual domain.
Profile directions and seams are retained; they are not automatically flipped.

The sweep runs from the first profile's station to the last profile's station.
A single profile instead sweeps forward from its station to the rail's end.
Inputs, attributes, groups, and selection remain unchanged. The new unselected
B-rep uses the current layer; profile creases split into shared-topology faces.
The operation is one undo step. Invalid options/construction do not modify the
document. Closed profiles are supported; closed rails are not.

| Option | Implemented behavior |
| --- | --- |
| `RefitRail=No` (default) | Interpolate profiles in the existing rational rail basis when their stations are already Greville sites; otherwise refit before inserting interior stations. |
| `RefitRail=Yes` | Refit the rail to cubic arc-length form, then interpolate profiles in that fixed basis. |
| `FrameStyle=Freeform` (default) | Shared rotation-minimizing frame transport. |
| `FrameStyle=Roadlike Axis=0,0,1` | Keep a frame direction perpendicular to both rail tangent and fixed axis; default axis is world Z. Parallel tangent/axis is an error. |
| `GlobalShapeBlending=No` (default) | Local cubic smoothstep between neighboring profiles, by rail arc length. |
| `GlobalShapeBlending=Yes` | Linear arc-length blending between neighboring profiles. |

For two profiles at native stations 0 and 5, select them in that order and use
`Sweep1 RailName=Rail Parameters=0,5 RefitRail=Yes`. The native model's blend
definitions preserve every supplied section, but do not reproduce all Rhino multi-section
results; see diagnostics below.

## Geometry and numerical method

`Sweep1::try_new` receives a `CurveRef`, ordered `SweepSection` records,
`SweepFrameStyle`, `SweepBlend`, and `Tolerance`. `sections_at` returns exact
rational cross-sections of the chosen numerical frame/blend model at an ordered
batch of native parameters. Each batch is seeded at the same swept start.
The [frame integrator](curve-frames.md) uses an angular target at most `1e-10`,
further tightened according to profile radius and construction tolerance.
Repeated arc-length queries reuse integrated prefix brackets, then integrate
the remaining interval; the lookup does not replace integration with linear
interpolation.

`section_basis` performs geometry-preserving weight scaling, active-domain
clamping, degree elevation, normalized parameter domains, and exact knot-union
matching. Loft reuses this module but retains its separate subsequent Rhino
end-weight policy. Sweeps blend local homogeneous section controls and weights
in the transported frame. Positive profile weights are required.

`to_rail_basis_surface` uses the trimmed rail's degree, knots, and weights
(up to a geometry-preserving common scale).
It evaluates transported sections at rational Greville stations mapped back
to native rail parameters, scales homogeneous sections by the rail denominator,
and solves the corresponding collocation system. When no refit is necessary this preserves the rail basis,
**not** rigid section motion between those stations. Unrefitted and refitted
spatial sweeps can therefore have different geometry, not merely different
control counts. Both construction paths accept multiple profiles. Public Rhino
outputs show that even `RefitRail=No` may refit if an interior section is not
already at a rail Greville site; the native implementation follows this rule.

`to_surface` first uses the shared curve fitter to approximate the rail as a
nonrational cubic, parameterized by integrated arc length, at one quarter of
the requested absolute tolerance. Missing interior section sites get symmetric
knot neighborhoods bounded by adjacent knots and sections. These are inserted
in section order. For a straight rail `[0,5]` with profiles at `0,2,5`, the
interior knots are `1,2,3`; with profiles at `0,1,2,5`, they are
`0.5,1,1.5,1.75,2,2.25`. The supplied stations become Grevilles without splitting
the rail into unrelated patches. Transported homogeneous sections are then
interpolated at that fixed basis's Grevilles. Local smoothstep is exact at those
sites, **not** throughout each intervening span. This distinction fixes the
previous three-profile Local construction difference.
Every supplied section is checked against the resulting positive-weight profile
basis, allowing only a common projective scale. A failed constraint is an error;
in particular, near-equal Greville classification on a large shifted parameter
domain cannot silently erase a profile. This constraint audit does not bound
the surface between sections.

`fit_model_surface` separately adaptively fits cubic U control trajectories of
the continuous numerical model; it is not the command's refitted path. Section stations
are retained with C1 local or C0 global blend joins. At each refinement level,
uniform and cosine-offset U checks compare complete rational section bases.
Positive weights bound all V points at each checked U using control-point
error plus the profile diameter times relative weight error. U auditing is
sampled, not a certified continuous bound. Failure to reach the requested
accuracy, nonpositive fitted weights, or exhausted parameter resolution is an
error. Construction limits are 256 profiles, 512 compatible profile controls,
512 rail-fit controls, 1,024 final U controls, and 262,144 total surface controls.

All paths share `spline_collocation` with the morph fitters: cubic systems use
banded factorization, other degrees use dense full-pivot `faer` solves, and
multiple right-hand sides share the factorization. Homogeneous fitting
subtracts a local origin before solving. V is normalized to `[0,1]`. Refitted U
uses integrated arc length; unrefitted U uses its rational representation's
parameter; the continuous-model fitter retains native U. Rhino's refitted U
uses an approximate length parameterization, and its V
domain can retain the original profile domain. Equal normalized UV coordinates
are therefore not a valid cross-implementation geometry comparison.

## Verification and known differences

Independent native tests cover straight local/global blends, one- and two-profile
analytic circular sweeps, off-grid fitting errors below `1e-9`, exact supplied
rational sections, multi-profile knot placement, Roadlike orientation, unrefitted rail preservation and
Greville interpolation, common rational rail-weight scales `1e±280`, invalid
inputs, and forward/inverse arc-length queries. Command tests check multi-profile
blending and both frame styles, closed and creased profile topology, retained
document state, and undo. A failing-then-fixed closest-point regression checks a skew bilinear
surface: clamping a coupled Newton step could stall before the minimum along
an edge. All four natural boundary curves are now searched independently in
addition to the bounded multi-start interior search. This does not certify a
global minimum on arbitrary surfaces. Tight arc/spatial curve-fit regressions
sample independent off-grid points, and a nonuniform/kinked cubic collocation
test compares the banded solve against independent dense elimination.

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

Each of the 39 cases compares 135 unrounded closest-point results: 81 original
Rhino surface-grid points plus 54 world-axis offsets from corners, edge
midpoints, and the center. These sample geometry and extents but are not a
bidirectional continuous distance certificate. Command probes compare output
surface geometry only, not Rhino selection/attributes, and report zero timing
to indicate untimed command execution.

| Fixture | Cases | Result at absolute `1e-6`, relative `1e-12` |
| --- | ---: | --- |
| `sweep1.json` | 11 | Pass; maximum coordinate difference `2.75e-7`. |
| `sweep1_command.json` | 6 | Pass; maximum `2.32e-7`, including both refit choices. |
| `sweep1_multisection.json` | 10 | Pass; maximum `4.82e-8`, including Local three/four profiles, close stations, retained degree-five/arc bases, and two-profile Global commands. |
| `sweep1_curved_blend.json` | 2 | Fail: two-profile arc `1.05e-6`, spatial rail `1.28e-6`. |
| `sweep1_diagnostics.json` | 2 | Fail: rational closest-point results `0.0457`, three-profile Global `0.198`. |
| `sweep1_basis_diagnostics.json` | 8 | Fail: unrefitted spatial/rational profiles, Global commands, refit degree/parameter differences, and near-boundary closest-point differences. |

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
does not resolve unrefitted rational profile normalization or certify general
surface closest-point searches. The five-profile boundary diagnostic includes
a native result farther from the query than Rhino's; both surface searches
remain bounded numerical algorithms.

```sh
tools/rhino_oracle/run_headless.sh compare tools/rhino_oracle/fixtures/sweep1.json \
  --absolute-epsilon 1e-6 --relative-epsilon 1e-12
tools/rhino_oracle/run_headless.sh compare tools/rhino_oracle/fixtures/sweep1_command.json \
  --absolute-epsilon 1e-6 --relative-epsilon 1e-12
tools/rhino_oracle/run_headless.sh compare tools/rhino_oracle/fixtures/sweep1_multisection.json \
  --absolute-epsilon 1e-6 --relative-epsilon 1e-12
```

Closed-rail closure, rail miters, complete unrefitted multi-profile compatibility,
automatic section placement/seam alignment, viewport picking, and remaining
Rhino rebuild/refit options are not implemented. These are explicit limits,
not evidence that the overall project goal is complete. Performance is also
unfinished: short release timings for the spatial refitter are tracked separately
from geometry comparisons; dense-to-banded solving alone did not remove the
cost of repeated accurate arc-length inversion.
An exact-key, bounded sample cache reduced single-profile spatial timings from
about 43 ms to 25 ms, versus 15 ms Freeform and 12 ms Roadlike in Rhino under
FEX/Wine. Two-profile spatial construction measured 26 ms versus 19 ms.
Most simple cases were faster natively, but these separate short runs are not controlled benchmarks
and do not satisfy the project's general performance requirement.
