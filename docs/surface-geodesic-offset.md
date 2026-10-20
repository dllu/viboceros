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
`1.23e-5`, alongside the five existing SDK discrepancies. These are sampled
comparisons, not continuous geometric certificates. ThroughPoint, curved-surface
open-end extension, general closed-gap reconnection, corners, cut-locus
handling and full native parity remain pending.

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
Append the command replay output to that log, then regenerate the qualification report with
`python3 -m tools.rhino_oracle.report_surface_geodesic path/to/replay.log`.
