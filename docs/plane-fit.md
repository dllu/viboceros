# Best-fit plane projection

[Align command](commands/align.md) · [Numerical policy](numerical-robustness.md)

`PointProjection3::onto_best_fit_plane(&points)` minimizes the sum of squared
orthogonal point distances. Every occurrence has equal weight. Its result is a
prepared plane projector, not a flattened copy of the input geometry.
`Align ToFitPlane` uses one tight-box bottom-center anchor per selected object;
it then translates each whole object independently, without treating groups as
rigid units. At least three selected objects are required by the command.

## Numerical design

The kernel accepts any nonempty set, up to `MAX_PLANE_FIT_POINTS` (one million).
Exact affine-rank checks return a containing plane for coincident, collinear,
or coplanar inputs. Axis-constant data has a direct, allocation-free scan.
Coplanar inputs therefore remain exactly fixed, including very thin and
extreme-range configurations that floating-point rank thresholds could misclassify.

Full-dimensional data uses an exactly centered, isotropically scaled N-by-3
matrix and faer's thin SVD. Scaling all three axes together preserves the
Euclidean least-squares objective; scaling them independently would not.
The smallest right singular vector supplies the numerical normal. Projection
retains the **exact rational centroid**, not an already-rounded floating-point
origin. `FiniteSum::mean` also exposes correctly rounded means whose totals
may overflow binary64; empty means are errors.

The normalized matrix uses a compact shared binary quantum and integer
denominator. Each entry is rounded once, without repeated rational GCDs.
Regression tests compare these entries bit-for-bit with an independent
per-entry rational calculation over several sizes and exponent ranges.
Other tests check orthogonal scatter eigenvectors, duplicate weighting,
permutation invariance with separated normals, exact planar preservation,
centroid cancellation, and explicit resource/range failures.

This is a numerical SVD, not an exact algebraic eigensolver. Full-dimensional
sets whose normalized coordinates underflow or become subnormal are rejected;
an unresolved/nonfinite full-rank decomposition is also an error. Nearly tied
smallest singular values can make the normal sensitive to input rounding.
Exactly tied values permit multiple equally optimal planes. There is no claim
that the chosen basis matches Rhino's basis in those cases.

## Command lifecycle

`Align ToFitPlane [AlignTo=CPlane|World]` executes without reference-point picks
after object selection. `Auto`, point arguments, and `3Point` are invalid.
Exactly planar anchors produce a no-op with no geometry-history entry. All
replacements are staged before one atomic document edit; object IDs, geometry
types, attributes and group memberships survive.

Rhino's actual command accepts three anchors, including collinear/coincident
ones. With fewer than three objects it reports failure without changing
geometry. Preselection survives; command-first selection is released. The
command registry's optional failed-postselection cleanup runs **after** a
failed transaction rolls back, so that cleanup cannot be undone by rollback.
Other errors retain their existing selection and atomicity policy. CPlane/world
choice is command-instance state outside undo history.

## Evidence and remaining differences

The 39 [regular fixtures](../tools/rhino_oracle/fixtures/align_fit.json) and their
[Rhino 8.32 records](../tools/rhino_oracle/observations/align_fit.json) cover
minimum selection, degenerate data, duplicate weights, pre/postselection,
overlapping and partial groups, World/CPlane anchors, mixed objects, rational
and signed curves, conics and polycurves. Raw acceptance remains absolute
`1e-8`, relative `1e-12`; all value and document-state fields are compared.
The [combined executable record](align-comparison.json) retains hashes and all
passing and failing Align cases. Rhino runs only in owned private Xvfb sessions.

Six [fit diagnostics](../tools/rhino_oracle/fixtures/align_fit_diagnostics.json)
have [unmodified measurements](../tools/rhino_oracle/observations/align_fit_diagnostics.json):

- Two mixed mesh cases differ by up to `5.83e-8`. Every mesh coordinate agrees
  exactly after conversion of the native coordinate to Rhino's binary32 storage;
  all remaining fields are checked without conversion.
- The tetrahedron and two octahedron cases have isotropic scatter. Rhino and
  the native solver choose different equally optimal planes, causing coordinate
  differences up to `1`. Independent checks establish coplanarity, orthogonal
  motion and the minimum residual sum for both outputs; raw parity still fails.
- The oblique paraboloid disk inherits the existing tight-bound discrepancy,
  with fitted-output differences up to `5.41e-5`. Its anchor is independently
  checked from analytic extrema: for radius `r=.8`, X=`u+2v+3z` ranges from
  `-5/12` to `3r²+r√5`, Y=`-2u+v` is symmetric, and Z=`-3u-6v+5z` has minimum
  `-9/4` at `(u,v)=(3/10,3/5)`, where `z=u²+v²`.

No native coordinates are rounded or comparison thresholds widened to absorb
these differences. Full Align parity, grip/component alignment and `ToCurve`
remain unfinished.

## Local performance

```sh
cargo run --release -p viboceros-geometry --example profile_plane_fit
```

The bounded example measures whole fit construction, including rank checking,
centering and SVD, on 100- and 10,000-point sets. A DGX Spark release run gave
these medians of three calls:

| 10,000-point input | Initial per-entry rational calculation | Shared-denominator implementation |
| --- | --- | --- |
| Noisy oblique plane | 67.6 ms | 5.96 ms |
| Exact axis plane | 34.2 ms | 0.004 ms |

The first column predates both shared-denominator normalization and the exact
axis-plane fast path. These local measurements do not establish Rhino kernel
or end-to-end command performance parity. General oblique coplanarity checks
and exact projection still have optimization opportunities.
