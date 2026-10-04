# Circle fit kernel and native evidence

[Curves](commands/curves.md) · [Architecture](architecture.md) · [Oracle](oracle.md)

`Circle3::try_fit_to_points` prepares a least-squares plane through the point
centroid, then fits a center in that plane and a radius. Its objective uses
**full spatial distances from the center**, including heights above the plane.
It retains occurrence weights. Coincident and collinear inputs return `None`;
fewer than three points return an error. Input count and refinement are bounded.

The implementation uses correctly rounded centroid means, isotropic scaling,
faer's thin SVD for the plane, a covariance initial estimate on well-conditioned
data (a Taubin SVD estimate for thin arcs), and damped
Gauss–Newton refinement with nalgebra for ordinary centers. Distant circles
retain the algebraic center and use stable mean spatial radii; see the
[conditioning evidence](circle-fit-conditioning.md). Accepted steps retain their statistics
to avoid repeating distance and derivative scans. Eliminating the radius as the mean
spatial distance leaves two center coordinates to refine. A finite orthonormal
frame and checked Circle bounds validate the output.

## Evidence

22 owned Rhino **8.32.26160.13001** recipes compare public
[Circle.TryFitCircleToPoints](https://developer.rhino3d.com/api/rhinocommon/rhino.geometry.circle/tryfitcircletopoints)
with actual `Circle FitPoints` commands. Coverage includes three points, full
circles, short arcs, radial and axial noise, translated centers, repeated points,
collinear/coincident inputs, and a nearly collinear diagnostic. Native command
frames and radii equal the public SDK outputs for all retained constructed
circles. Snapshots retain command-end selection and Undo/Redo.

The spatial-distance objective is an inference from those controlled outputs.
For the axial-noise examples, fitting only projected distances differs in
radius by up to `0.0761`; spatial distances agree with the measured radius.
The nonlinear optimizer's independent stationary solutions differ from native
centers by roughly `1e-8`, so regular fitted **loci** are compared at `1e-7`.
This is a geometric comparison; it does not assert matching parameters.

The [provenance record](circle-fit-provenance.json) hashes the closed recipes,
unmodified observations and helpers. All Rhino work uses owned private Xvfb.
No proprietary source was inspected.

44 additional public circle/plane API captures cover nearly collinear data,
noisy thin arcs and symmetric sets containing their fitted center. Every native
Circle frame exactly matches the public plane fitter's frame. The original
large near-collinear mismatch is now within sixteen radius ULPs; a different
thin input retains a small unresolved discrepancy. Details and numerical bounds
are in the [conditioning record](circle-fit-conditioning.md).

## Local performance

The [retained performance record](circle-fit-performance.json) measures prepared
100- and 10,000-point inputs with radial and axial noise, ten warmups and the
median of three complete fitting calls. .NET point-array construction is outside
the native timer; Rust point construction is outside its timer too.

| Points | Rust fitter | Rhino SDK under FEX |
| --- | ---: | ---: |
| 100 | 0.0332 ms | 0.4835 ms |
| 10,000 | 1.206 ms | 2.017 ms |

The native 100-point timings range from 0.0443 to 0.8186 ms and show large
outliers; the raw values are retained. The 10,000-point Rust fit initially
took 2.219 ms. Reusing accepted-step statistics and using a compact seed solve
on well-conditioned data reduced it to 1.206 ms without changing the retained
loci. These measurements do not establish performance against untranslated
Rhino or overall command performance.
These are the original baseline measurements. Current conditioning checks
retain both later runs and their timing variation in the
[conditioning record](circle-fit-conditioning.md).

```sh
cargo run --release -p viboceros-geometry --example profile_circle_fit
```

## Remaining compatibility work

This is a fitting foundation, and **full native parity remains incomplete**.
Native fitted normals can flip with the input data, and seams depend on the
resulting plane basis. Solver bases are not yet a compatible substitute for
those conventions. Circle's interactive FitPoints input, control-point/mesh
vertex picking, and Maelstrom's FitPoints input still require this work.

The diagnostic with a `1e-6` departure from collinearity retains a roughly
`2.35e-6` difference in both center and radius. It remains an explicit failing
compatibility case, separate from the regular-fit and floating-point bounds.

Original native performance, tied plane singular values, extreme coordinate
ranges and nonsymmetric sets containing the fitted center need further verification.
The fitter checks finite output bounds and rejects failed decompositions or
exhausted refinement. A stalled descent can return its current finite estimate;
this is not a certificate of a global minimum or native convergence behavior.

```sh
cargo test --release -p viboceros-geometry circle_fit
python3 -m unittest tools.rhino_oracle.test_circle_fit
tools/rhino_oracle/run_headless.sh rhino \
  tools/rhino_oracle/fixtures/circle_fit_points.json \
  --scheme VibocerosOracleCircleFit --output /tmp/circle-fit.json
```
