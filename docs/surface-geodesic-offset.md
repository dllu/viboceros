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

The accumulated numerical estimates are not a global geometric certificate.
Very ill-conditioned surfaces, arbitrary chart transitions, trimmed-face
boundaries and paths through poles need further work.

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

| Cases | Maximum measured difference |
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

## Command work still required

`OffsetCrvOnSrf` is not registered yet. The remaining work includes resolving
the five SDK differences, fitting complete offset curves, trimmed faces and
surface-edge inputs, Number and ThroughPoint behavior, temporary source
portions, previews and grouped history.

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
cargo test -p viboceros-geometry --lib cash_karp
cargo test -p viboceros-geometry --test surface_geodesic_native -- --nocapture
python3 -m unittest tools.rhino_oracle.test_surface_curve_offset
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.generate_surface_curve_offset
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.generate_surface_curve_offset --verify
```

To refresh the independent query points, save the Rust replay output and pass
it to the capture generator with `--candidate-report path/to/replay.log`.
Regenerate the qualification report from the same output with
`python3 -m tools.rhino_oracle.report_surface_geodesic path/to/replay.log`.
