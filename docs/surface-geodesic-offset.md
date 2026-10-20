# Surface distance and OffsetCrvOnSrf foundation

`PreparedSurfaceGeodesic` prepares an immutable NURBS surface for repeated
surface-distance queries. `point` takes a native UV location, a tangent direction,
a signed distance and model tolerance. It returns the endpoint, native UV,
travel tangent, integration diagnostics and endpoint encoding residuals.

The preparation normalizes both parameter axes and physical coordinates before
evaluating derivatives. A thin QR solve supplies the geodesic acceleration
without squaring the metric's condition number. Coupled Cash–Karp steps extend
the same embedded rule used by curve-length queries. Accepted steps retain
position/velocity error estimates and check arc-length speed drift. C1 charts
can cross periodic seams and second-derivative breaks. Natural boundaries,
singularities, non-C1 chart transitions and unrepresentable endpoints return
errors; they do not return a shorter path as the requested result.
Periodic wrapping requires identical boundary definitions; a thin open strip
does not become periodic through an approximate object-closure predicate.
Exactly affine bilinear charts use a direct metric step, retaining domain and
endpoint precision checks. Exact stored coefficients decide this fast path;
a nearly planar patch keeps its geodesic integration.

The accumulated numerical estimates are not a global geometric certificate.
Very ill-conditioned surfaces, arbitrary chart transitions, trimmed-face
boundaries and paths through poles need further work.

`try_offset_curve_on_surface` fits a complete offset locus for a smooth source
on an untrimmed regular surface. A certified pullback first admits the source;
the shared adaptive morph collocation then fits geodesic endpoints in the
source parameter chart. It retains separate tangents at source self-crossings,
one-sided knot values, reduced derivative continuity, and exact closed seams.
Rational composition candidates reuse the morph fitter's homogeneous solves
before polynomial refinement. A caller may supply its existing interpolation
producer through `try_offset_curve_on_surface_with_solver`.

The result retains the original parameter domain, source deviation bound,
query/step counts and maximum integration estimate. It rejects natural-boundary
crossings, discontinuous offsets and unrepresentable restored parameters.
Fitting is checked at both uniform and nonuniform stations after every
refinement; this does not certify all points of an arbitrary offset locus.

`try_offset_curves_on_surface` returns the portions inside the natural
rectangle. It reuses endpoint-span surface extension, adaptive UV fitting,
curve intersection and exact curve trimming. UV crossing proposals get
independent model-space contact checks, and retained portions get additional
fit and membership checks. Periodic seams remain traversable; separate
portions of a closed source join across its old seam. Empty output is distinct
from numerical failure. Sources remain immutable.

Clipping currently requires same-sign surface weights. Temporary extension
has eight bounded retries; singular continuations and unresolved contacts
return errors. UV fit tolerances use a conservative natural-chart derivative
bound, with model-space validation after clipping. These sampled checks do not
certify topology of the complete geodesic family.

`SurfaceCurveOffsetOptions::closed_gaps = ConnectClosed` requests the captured
SDK's planar closed-gap behavior. Retained portions are reconnected by
parabolic spans using their endpoint tangent intersection. Those spans can
leave the natural rectangle. This policy is qualified for affine planar
patches; unsupported closed reconnections return errors. The default `Open`
policy retains separate clipped portions. Reconnection uses unit bridge
parameter spans where representable and source-period units otherwise,
checking the assembled geometry after parameter mapping.

## Evidence

Analytic tests cover skew planes under world scales from `2^-200` to `2^200`
and parameter scales from `2^-600` to `2^600`, thin skew charts, cylinder helices
and seam crossings, oblique great circles on spheres, reversal, boundaries,
creases and world/parameter roundoff. A separate Cartesian integration on the
implicit warped patch checks the UV formulation without NURBS derivatives or
the metric solve.

The [24 public SDK captures](../tools/rhino_oracle/fixtures/surface_curve_offset.json)
use plane lines and Bézier profiles, cylinder generators and rings, sphere
latitudes and a warped bilinear patch, with signed distances 0.25 and 1.
Each input exports its full NURBS definition, and the SDK sources remain
unchanged. Seven source stations per case give 168 geodesic shots. Comparisons
measure closest-locus distance to the captured SDK offset curves.

The `1e-6` qualification gate is unchanged. Nineteen case recipes meet it,
including the expected natural-boundary rejection. Five remain unqualified:

| Source-station cases | Maximum point-shot difference |
| --- | ---: |
| Bézier profile, distance ±1 | `3.12e-6` |
| Warped patch, distance ±0.25 | `2.63e-5` |
| Warped patch, distance +1 | `1.05e-4` |

[Independent SDK closest-point queries](../tools/rhino_oracle/fixtures/surface_curve_offset_verify.json)
check 42 saved candidate points on six cases. Their distances agree with the
Rust closest-point query within `1.3e-15`; the discrepancies are present in the
captured offset curves. The replay records diagnostic ceilings for these five
cases separately from the parity gate. It must not be reported as full native
offset compatibility. See the [qualification report](surface-geodesic-qualification.json).

Clipped offset production also replays all 24 cases. It produces 23 curves,
compared at 65 samples in each direction, and one empty result. Nineteen recipes
qualify; the five cases above remain numerically unqualified. The negative
0.25 warped offset now clips to both natural U boundaries and shortens its
parameter interval, matching the structure of the SDK output.
The complete distance-1 Bézier comparisons reach `3.70e-6` sampled difference.
Separate dense analytic tests cover fitted planes, source self-crossings, cylinder generators,
closed rings, sphere latitudes, physical and UV rescaling, and reversal. The
warped-patch full curve is checked against an independent Cartesian equation.

Five additional [clipping captures](../tools/rhino_oracle/fixtures/surface_curve_offset_clipping.json)
cover four separate circle arcs, a tangent circle, a seam-crossing arc, both
ends of an oblique line, and an empty result. The explicit closed-gap policy
matches the five SDK outputs within `1e-6` at 129 samples in each direction.
Open mode also verifies the separate natural-domain portions. Analytic tests
cover tangencies, boundary overlaps, periodic seams, source seam joins and
scaling from `2^-100` to `2^100`, including parameter precision during closed
gap reconnection.

## Command integration and remaining work

[`OffsetCrvOnSrf`](commands/offset-crv-on-srf.md) is registered with scripted
and viewport source/face input, signed distances, picked sides, Flip, Number,
temporary directed SubCrv input, cached previews and atomic history. Existing
face clipping retains outer trims and holes. Planar open outputs reuse tangent
boundary extension, matching the public command's behavior rather than the
SDK's retained source endpoints. Number advances the complete offset before
clipping each copy against face loops.

Eight additional public command captures replay geometry, selection, layers,
names and Undo/Redo through both command and UI paths. Seven qualify within
`1e-6`. The extended Bézier offset remains unqualified at approximately
`4.24e-6`, alongside the five existing SDK discrepancies. These are sampled
comparisons, not continuous geometric certificates. Curved-surface
endpoint-continuation parity, general closed-gap reconnection, corners, cut-locus
handling and full native parity remain pending.

ThroughPoint inverts the same normal-geodesic family in source-parameter and
signed-distance coordinates. It reuses the shooting preparation, source-image
admission and thin QR solve. A bounded search starts at the Euclidean closest
foot, then checks parameter cells; same-sign control hulls exclude cells whose
chord lower bound exceeds an accepted intrinsic distance. Backtracking never
accepts partial shots at boundaries. Singular target charts, ambiguous equal
normal branches and exhausted work fail explicitly. The resulting fitted and
face-clipped curve must independently pass through the constrained point within
model tolerance. Global root completeness and cut-locus certification remain
unproven. Repeated UI points retain accepted curves in one history entry.

Curved open inputs continue along the endpoint normal section: the plane
spanned by the source tangent and surface normal. The implementation reuses
existing exact surface/plane intersections, selects the local oriented branch,
and stops at the first natural rectangle edge or half of a closed section.
Intersection queries normalize physical coordinates as well as UV domains.
Unsupported sections fail before model edits. This policy is inferred from
public SDK captures; it does not inspect proprietary code. Explicit UV span
continuation remains available as a separate geometry operation.

With `native-smlib`, endpoint continuation can use the existing usd-brep
surface intersector for general NURBS patches. The portable analytic producer
runs first; its specific restrictions keep their measured policies. Native
trace candidates use the shared arc-length sampler and constrained spline
fitter. Complete source-surface image bounds and exact rational plane-band
checks admit the result, including its stored controls after degree elevation
and joining. Raw intermediate trace endpoints must reach a natural boundary;
a partial component cannot silently become a completed continuation.

A conservative UV bound can reject a physical boundary contact already within
the numerical allowance. These cases now use an exact stored-curve endpoint
distance bound before requesting a tiny section. The original document source
remains unchanged.

Four further native scenes cover a degree-2-by-2 patch, reversed input, an
isocurve and a rational patch. Both command and source-extension comparisons
retain the `1e-6` gate. Two command cases qualify, with maximum differences of
about `8.7e-7` and `5.2e-7`. The reversed command remains unqualified at
`1.04e-6`; the isocurve differs by `2.15e-5`. Every source-extension endpoint
pair agrees within `1.6e-15`, while three captured SDK continuation curves
leave their surface by `8.5e-6` or `2.03e-5`. Their complete comparisons remain
unqualified.

The independent [graph-metric calculation](../tools/numerics/check_general_surface_geodesic.py)
integrates the isocurve's normal geodesic without the Rust NURBS evaluator.
A fresh [SDK query](../tools/rhino_oracle/observations/surface_curve_offset_general_verify.json)
places that point `1.19e-5` from the SDK offset of the extended source, versus
`2.83e-7` from its offset of the original straight source. The Rust result
passes through the independent point within model tolerance. These sampled
checks do not certify complete offset topology or global cut loci. The replay
constructs documents with the captured `1e-6` model tolerance; the default for a new document is `1e-9`.

Tangent-discontinuous sources on affine planar patches reuse the planar
curve-offset producer, including optional usd-brep structural operations.
Inside corners retain its intersection/trim policy. Outside connectors replace
its identified circular bridges with polynomial quadratics through their
tangent apex. Nine public SDK scenes demonstrate this policy: a right-angle
bridge at distance `0.5` differs from a quarter-circle by about `0.03033`.
Eight polyline scenes agree within `6e-14`; the mixed line/Bézier scene agrees
within `2.2e-7`. Original source admission precedes this branch, and admitted
normal displacements are projected through the certified UV witness. The
constructed locus has its own parameter clock; boundary contacts therefore
intersect that locus, independently of the smooth-source geodesic sampler.
Open straight command results use chord-length parameterization after endpoint
extension; closed inward polygons retain surviving source-interval durations.
Shared interval assembly retains closed seams, and every retained portion
must pass a complete image bound on the original surface. Eight command/getter
scenes meet the unchanged `1e-6` gate. The mixed curve command remains
unqualified at `3.18e-5`. An independent Bernstein point at `s=5/8` is exactly
`(0.4296875, -2.3046875, 0)`; fresh public SDK queries place this point about
`1.89e-7` from the SDK bridge and `3.18e-5` from the command bridge. The
current bridge passes through it within `1e-9`. This separates the native
command discrepancy from the unchanged model gate. Corner continuation
on curved surfaces, offset collapse and general self-intersection topology
remain open.

Certified pullbacks retain separate UV limits across exact periodic seams,
including crossings inside a rational span, multiple laps, and simultaneous
torus seams. Every assembled periodic pullback receives a complete image
proof. Surface closure requires matching boundary control nets; a small open
gap does not become periodic through model tolerance. The command emits seam
portions independently while retaining the complete curve for Number and
subsequent geometry operations. Each returned portion and restored native
parameterization retains an independent surface-image proof.

Eight public SDK source-extension scenes now replay against this policy.
Seven meet the unchanged `1e-6` sampled gate. The diagonal cylinder source
matches both endpoints, but its captured native continuation drifts about
`2.9e-6` off the exact cylinder. Independent cylinder unrolling also shows
that the captured SDK offset starts about `2.45e-3` before the exact
normal-family endpoint; the command's following extension inherits that gap.
These observations remain separate from the parity gate. The command replay
checks all eight scenes, including the two periodic diagonal portions,
attributes, source immutability and Undo/Redo. Five command scenes qualify; the
warped diagonal improved from about `0.282` to less than `4e-7`. Detailed
current measurements are in the [qualification report](surface-geodesic-qualification.json).

Open curved offset fits constrain endpoint tangent directions using the
arriving geodesic direction and surface normal. A one-sided estimate supplies
the signed speed, preserving reversals at focal points. Analytic cylinder
unrolling checks these directions independently. This prevents a free-fit
tangent error from growing over a long normal section. ThroughPoint shares
source preparation across picks and unchanged options.

The first ThroughPoint getter also accepts `Select` for ordered point objects.
The same inverse and fitting path prepares the complete batch before one atomic
commit. Point captures reject stale geometry and unselectable inputs. Eight
further native captures replay selection order, off-surface projection, skipped
on-source points, duplicate offsets from distinct points, cancellation and
history. All five output cases meet the unchanged `1e-6` sampled gate, with a
maximum difference of about `2.2e-14`. Cancel, empty selection and an all-on-source
batch create no geometry or Undo step. The point getter releases selection;
its local highlights and selection windows leave geometry and Redo untouched.

Sixteen native ThroughPoint scenes compare repeated picks, constraints,
selection, layers and grouped history through both command and viewport
getters. Fourteen qualify at `1e-6`. Bézier and cylinder cases remain
unqualified. Nine [independent SDK closest queries](../tools/rhino_oracle/fixtures/surface_offset_through_verify.json)
measure the captured Bézier curve `3.89e-3` away from an analytic normal-family
point near a fold, and cylinder outputs up to `3.49e-5` from their requested
points. The model gate remains unchanged. These measurements establish the
discrepancy in the saved curves without assuming Rhino's approximation method.
Trimmed-face ties retain captured topology and prefer the higher edge index;
rotation, U/V reversal and splitting one tied edge qualify this behavior.

Open affine-planar fits now impose analytic endpoint derivatives through
fixed control rows in the shared collocation layer. An exact affine lift of
the admitted UV source supplies its projected tangent and curvature, including
spatial sources admitted slightly off the plane. Rational candidates must also
match these endpoint derivatives. Position validation and refinement retain
their existing gates; other morphs keep their original interpolation policy.

The existing usd-brep `SmBSplineSurface::OffsetCurveAlongSurface` producer
intersects Euclidean offset circles with the surface. That metric differs from
Rhino's surface distance: on a radius-3 cylinder, a Rhino offset of 1 advances
approximately `1/3` radians, whereas a chord-distance offset advances
`2 asin(1/6)`. The implementation therefore cannot delegate this command to that
producer unchanged. The public reference is
[Curve.OffsetOnSurface](https://developer.rhino3d.com/api/rhinocommon/rhino.geometry.curve/offsetonsurface?overload=2)
and the [command workflow](https://docs.mcneel.com/rhino/8/help/en-us/commands/offsetcrvonsrf.htm).

## Reproduce

```sh
cargo test -p viboceros-geometry --lib surface_geodesic
cargo test -p viboceros-geometry --lib surface_curve_offset
cargo test -p viboceros-command --features native-smlib --lib surface_curve_offset
cargo test -p viboceros --features native-smlib --bin viboceros surface_curve_offset
cargo test -p viboceros-geometry --lib cash_karp
cargo test -p viboceros-geometry --test surface_geodesic_native -- --nocapture
python3 -m unittest tools.rhino_oracle.test_surface_curve_offset
cargo test -p viboceros-command --test surface_curve_offset_native -- --nocapture
cargo test -p viboceros --bin viboceros surface_curve_offset
python3 -m unittest tools.rhino_oracle.test_surface_offset_command
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.generate_surface_curve_offset
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.generate_surface_curve_offset --verify
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.generate_surface_curve_offset --clipping
```

To refresh the independent query points, save the Rust replay output and pass
it to the capture generator with `--candidate-report path/to/replay.log`.
Append the command replay, the native command-library replay, and the native
UI replay to that log, each run with `-- --nocapture`. Then regenerate the
qualification report with
`python3 -m tools.rhino_oracle.report_surface_geodesic path/to/replay.log`.

The additional fixed scenes and independent graph witness can be reproduced with:

```sh
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.generate_surface_curve_offset --general-extensions
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.generate_surface_offset_command --general-ends
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.generate_surface_curve_offset --general-verify
python3 tools/numerics/check_general_surface_geodesic.py
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.generate_surface_curve_offset --corners
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.generate_surface_offset_command --corners
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.generate_surface_corner_verify
```

The independent graph calculation requires SciPy; the Rust and oracle replay tests do not.
