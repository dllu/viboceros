# Circle fitting conditioning and plane frames

[Circle fit](circle-fit.md) · [Oracle](oracle.md)

## Captured behavior

44 public API recipes were measured in owned private Xvfb sessions on Rhino
**8.32.26160.13001**. The main 38 recipes repeat the original 22 point sets and
add near-collinear offsets from `1e-2` to `1e-12`, scales `0.001` and `1000`,
axis permutations, center witnesses and repeated center witnesses. Six more
recipes cover thin arcs with radial noise and angular spans down to `0.02`.
The [provenance record](circle-fit-diagnostics-provenance.json) retains helper,
recipe and unmodified observation hashes.

Every native Circle uses exactly the X/Y axes and normal returned by
[Plane.FitPlaneToPoints](https://developer.rhino3d.com/api/rhinocommon/rhino.geometry.plane/fitplanetopoints)
on the same point set. Thus the remaining oriented frame problem is shared with
the public plane fitter. This observation does not determine its SVD or normal
sign algorithm; our numerical plane normal can still have the opposite sign.

Symmetric cardinal points of radius two plus one center point fit to radius
`1.6`; four repeated center points fit to radius `1`. Rhino retains the common
center even though its distance derivative is undefined at those witnesses.
The kernel now uses the zero subgradient there and includes zero distances in
the mean radius. Axially offset center points and a nonsymmetric near-center
point are also retained.

## Thin inputs

An ordinary Kasa covariance fit can underestimate the radius of a thin arc.
The original nearly collinear input previously produced a radius near
`71428571`, versus native `199999999.99999982`.

The independent thin-input estimate uses Taubin's gradient-normalized
algebraic objective, described in section 4.4 of
[Chernov and Lesort, *Least squares fitting of circles and lines*](https://arxiv.org/pdf/cs/0301001).
For centered plane coordinates `(x,y,z)`, let `q=x²+y²+z²` and `m=mean(q)`.
The smallest right singular vector of the rows

```text
[(q-m)/(2*sqrt(m)), x, y]
```

gives coefficients `(a,b,c)` after undoing the first-column normalization.
The center is `(-b/(2a), -c/(2a))`. The spatial squared distances include height
above the plane. Small coefficient components are recovered with a two-variable
eigenvector block solve and correctly rounded moment sums. This avoids dividing
absolute SVD vector errors by tiny curvature.

The use of this estimate to match native thin circles is an inference from
controlled outputs, not a claim about proprietary implementation. In all six
captured noisy thin arcs, geometric refinement moves away from native centers.
The kernel therefore retains its algebraic estimate when its normalized center
distance exceeds 16 input scale units. The thin seed is selected below a plane
singular-value ratio of `0.1`, or if the ordinary estimate is distant.
These switching thresholds need broader coverage before claiming full parity.

The distant radius is still the mean full spatial distance. To avoid cancelling
two very large values, use center length `L`, direction `n`, and `k=1/L`:

```text
h = k*|p|²/2 - dot(n,p)
distance(p,center) - L = 2*h/(sqrt(1+2*k*h)+1)
radius = L + mean(distance(p,center)-L)
```

Offsets are accumulated before adding `L`. A finite checked Circle validates
the result. The ordinary path retains bounded damped Gauss–Newton refinement.
Neither path certifies a global minimum.

## Comparison bounds and remaining gap

Regular fits retain the `1e-7` bound on center error plus radius error. Very
large circles use the larger of `1e-7` and **sixteen ULPs of native radius**.
This accounts for binary64 coordinate spacing; at radius `2e12`, one ULP is
`0.000244140625`. Normal comparisons remain unoriented. These bounds validate
geometric circles, not native seams, directions or command integration.
The [plane-basis diagnostics](plane-fit-basis.md) retain twelve independent
decomposition variants, and stricter vector/boundary tests guard the locus.
The [Maelstrom FitPoints captures](maelstrom-fit-points.md) verify that its
command maps use the fitted Circle frame, including reversed/tilted CPlanes.

Of the 44 API recipes, 41 positive-radius fits meet those bounds, two degenerate
sets return no Circle, and one case remains an explicit compatibility gap:

```text
[[0,0,0], [1,0,0], [2,1e-6,0], [3,0,0]]
```

Native radius is `1999999.999997051`; the independent estimate differs by about
`2.35e-6` in both radius and center. The test retains that disagreement and does
not enlarge the regular comparison bound to absorb it. Native oriented frames,
this convergence detail, arbitrary thin/noisy inputs, Circle control-point/mesh
vertex selection and Maelstrom FitPoints picking remain incomplete. Circle's
[point-object input](circle-fit-input.md) now replays native selection/history.

The current 10,000-point benchmark produced medians of `2.228` and `1.225` ms
in two invocations, versus the earlier retained translated SDK median of
`2.017` ms. Both current values are retained in the provenance record; they
show run-to-run variation and do not prove a consistent speedup or performance
against untranslated Rhino.

```sh
cargo test --release -p viboceros-geometry circle_fit
python3 -m unittest tools.rhino_oracle.test_circle_fit_diagnostics \
  tools.rhino_oracle.test_headless_cli
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle rhino \
  tools/rhino_oracle/fixtures/circle_fit_diagnostics.json \
  --scheme VibocerosOracleCircleFitDiagnostics \
  --output /tmp/circle-fit-diagnostics.json
```

The oracle CLI now accepts the documented private `--scheme` and JSON
`--output` options; live runs still require their own Xvfb display.
