# Sweep1 construction

[Command guide](../sweep1.md) · [Validation and limits](validation.md)

`Sweep1::try_new` receives a `CurveRef`, ordered `SweepSection` records,
`SweepFrameStyle`, `SweepBlend`, and `Tolerance`. `sections_at` returns exact
rational cross-sections of the chosen numerical frame/blend model at an ordered
batch of native parameters. Each batch is seeded at the same swept start.
The [frame integrator](../curve-frames.md) uses an angular target at most `1e-10`,
further tightened according to profile radius and construction tolerance.
Repeated arc-length queries reuse integrated prefix brackets, then integrate
the remaining interval; the lookup does not replace integration with linear
interpolation.

`section_basis` performs geometry-preserving weight scaling, active-domain
clamping, degree elevation, normalized parameter domains, and exact knot-union
matching. Sweeps apply one common weight scale to all profiles, preserving their
relative input weights; independently normalizing each profile would change the
refitted blend. Loft retains independent per-profile scaling. Positive input
profile weights are required.

| Placement policy | Rail retained | Rail refitted (including forced refits) |
| --- | --- | --- |
| Distance for blending | Rational rail parameter | Integrated arc length |
| Control positions | Euclidean interpolation | Homogeneous interpolation |
| Raw profile weights | Linear interpolation with the same blend fraction | Same |
| Blend fraction | Local smoothstep, for either option | Requested Local/Global model |
| After placement | Normalize every placed profile's end weights, then retain the first profile's transformed V knots | No end-weight normalization |

The order matters: normalizing inputs *before* blending produces a different
retained surface. End-weight normalization uses a locus-preserving projective
Möbius reparameterization, including near-equal weights; see the shared
[normalization policy and Rhino accuracy limit](../nurbs-numerics.md#endpoint-weight-normalization).
Retaining the first profile's transformed knots,
however, can change later multi-span profiles' geometric images when their
endpoint weight ratios differ. This measured Rhino policy is shared with Loft;
it must not be described as unconditional preservation of input profiles.

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
Every supplied section (after the retained path's explicit compatibility policy,
if applicable) is checked against the resulting positive-weight profile basis,
allowing only a common projective scale. A failed constraint is an error;
in particular, near-equal Greville classification on a large shifted parameter
domain cannot silently erase a profile. This constraint audit does not bound
the surface between sections.

A negative surface control weight need not imply a pole. Fixed-basis construction
checks each V control trajectory's scalar U weight function using convex-hull
bounds and subdivision, with a roundoff margin. Positive trajectory weights are
a sufficient bound for a positive tensor denominator. The original surface's
degree, knots and controls stay unchanged. Up to 4,096 subdivisions and 20 levels
are allowed; unresolved positivity, including tangent or crossing poles, is
rejected. This is a sufficient test, not support for every possible pole-free
mixed-sign representation. Zero-weight controls at infinity remain unsupported.

`fit_model_surface` separately adaptively fits cubic U control trajectories of
the continuous numerical model; it is not the command's refitted path. Section stations
are retained with C1 local or C0 global blend joins. At each refinement level,
uniform and cosine-offset U checks compare complete rational section bases.
Positive weights bound all V points at each checked U using control-point
error plus the profile diameter times relative weight error. U auditing is
sampled, not a certified continuous bound. Failure to reach the requested
accuracy, nonpositive fitted weights, or exhausted parameter resolution is an
error. Construction limits are 256 profiles, 512 compatible profile controls,
4,096 cubic rail-fit controls, 4,096 final U controls, and 262,144 total surface controls.

All paths share `spline_collocation` with the morph fitters: cubic systems use
banded factorization, other degrees use dense full-pivot `faer` solves, and
multiple right-hand sides share the factorization. Homogeneous fitting
subtracts a local origin before solving. V is normalized to `[0,1]`. Refitted U
uses integrated arc length; unrefitted U uses its rational representation's
parameter; the continuous-model fitter retains native U. Rhino's refitted U
uses an approximate length parameterization, and its V
domain can retain the original profile domain. Equal normalized UV coordinates
are therefore not a valid cross-implementation geometry comparison.
