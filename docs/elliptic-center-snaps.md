# Elliptical NURBS Center snaps

[Interface](interface.md) · [Circular Center audit](circular-center-snaps.md) · [Numerical robustness](numerical-robustness.md)

Center recognizes full and partial elliptical NURBS loci, including polycurve
leaves, natural surface boundaries and spatial B-rep edges. Capture follows the
original curve, not its supporting ellipse or the empty center. Targets preserve
model-space elevation. Circle-only radius and extension operations are unchanged.

## Recognition

`NurbsCurve::elliptical_center` decomposes the curve into rational Bézier spans.
An exactly accumulated control-point mean and a scaled planar frame keep fitting
coordinates near the data. Each span contributes the Bernstein coefficients of
`a X² + 2b XY + c Y² + 2d XW + 2e YW + f W²` to one six-column system.
Common weight gauges and equation rows are normalized; faer's SVD proposes its
null vector. Pooling all spans avoids depending on a short first span after
refinement. A positive-definite quadratic form gives the center and principal
axes through nalgebra. Degenerate, non-elliptic or numerically ill-conditioned
proposals are rejected, including nearly linear arcs with unstable centers.

The proposal alone never admits a snap. Every original span must have common-sign
weights, bounded plane distance and bounded Bernstein coefficients of
`Q·Q - W²` after mapping the ellipse to a unit circle. Half the model-distance
budget is reserved for plane error and half for radial error, scaled by the
maximum semi-axis. This reuses the circular recognizer's whole-span bound.
Neither sparse point sampling nor degree reduction establishes acceptance.

This is conservative floating-point recognition, not an exact algebraic
predicate or certified center-error interval. The conditioning guard is numerical;
short arcs, very eccentric ellipses, extreme weight spreads and precision-limited
coordinates can return no center. It does not implement Rhino's approximate-conic
option or establish unrestricted ellipse-recognition parity.

## Integration and evidence

The shared lazy Center slot tries circular recognition first, then elliptical
recognition. Mid remains independently lazy. Geometry/tolerance changes and Undo
invalidate both; failed results stay paired with their source. The UI's degree-32
recognition cap and existing projected proximity/clipping policy are unchanged.

The four previously unsupported ellipse captures in the retained 44-case
[circular audit](circular-center-snaps.md#retained-rhino-evidence) now replay full
ordered command geometry and Undo/Redo using independently computed native
centers. The perturbed quadratic has center `(3.75, -4, 0)` by the control-triangle
formula; the full ellipse has center `(4, -4, 0)`. Original requests, recorded
Rhino outputs and their hashes remain unchanged. This is a new native replay of
existing evidence, not a new Rhino run. The two negative-gauge differences and
four admission-only misses remain explicit.

Native regressions cover rotation out of XY, translated origins, refinement,
degree elevation, reversal, signed/extreme common weight gauges, unequal endpoint
weights, tiny/huge/translated parameter intervals and a nonlinear quartic with a
stationary endpoint. Rejections include parabolas, hyperbolas, mixed-sign spans,
nonplanar/altered later spans, ill-conditioned tiny arcs and a degree-nine bump
invisible to three sampled second-order jets. UI tests cover the four viewport
kinds, ordinary/constrained prompts, B-rep boundaries and a real off-plane click.

Verification checkpoint: 3,075 release-mode workspace tests, 282 Python tests,
seven offscreen GPU tests, formatting, and Clippy/Rustdoc with warnings denied.
The workspace's 25 opt-in tests remain ignored in the ordinary run; the seven
GPU tests were also run explicitly.

```sh
cargo test --release -p viboceros-geometry nurbs::ellipticity
cargo test --release -p viboceros-geometry nurbs::circularity
cargo test --release -p viboceros-drafting object_snap::cache
cargo test --release -p viboceros-oracle conic_nurbs_and_edges
cargo test --release -p viboceros viewport::drafting::center_tests
```
