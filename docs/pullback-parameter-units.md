# Pullback parameter units and UV borders

Valid on-surface curves could fail pullback after changing only UV units or the
source curve's parameter range. Endpoint snapping used an absolute UV-origin
scale with a minimum of one, which swallowed legitimate interior coordinates.
The regular fitter also tested derivative speeds against model distance and
formed native `dUV/dt` before multiplying by a span length.

Snapping now uses normalized distance within the UV interval. The fitter uses
the existing fractional, one-sided source-span sampler and scales model motion
before inverting the surface differential. Unit directions determine angular
admission; scaled parameter-step helpers avoid overflowing derivative speeds.
Native output knot intervals and source domains remain in use, and certificate
checks still compare against the complete original spatial curve.

`CreateUVCrv` now correctly reports the tested on-surface inputs as within
tolerance. Its border builder preserves each trim curve's local knots. Outer
polycurve intervals normally concatenate the native spans; if these intervals
collapse or overflow, bounded segment-index outer parameters retain every local
curve and its geometry. Native outer-domain parity for that unrepresentable
concatenation case is not established by these tests.

Tests use translated oblique trapezoid surfaces at scales `2^-20`, `1`, `2^40`,
`2^80`. Absolute tolerance is `1e-7` times scale. UV width exponent pairs are
`(0,0)`, `(-200,-200)`, `(200,200)`, `(-600,-600)`, `(600,600)`,
`(-600,600)`, `(600,-600)`. Source parameter widths include `2^-600`, `1`,
`2^600`.

| Matrix | Cases | Before successful | After successful |
| --- | ---: | ---: | ---: |
| Legacy/certified/fixed-endpoint pullback | 252 | 128 | 252 |
| Translated UV origins and near-boundary picks | 24 | 3 | 24 |
| CreateUVCrv output and history | 56 | 24 | 56 |

The independent analytic input is `S(u,v)=((10-2v)u,10v,0)` with
`u(t)=t`, `v(t)=1/5+7t/5-t²`, transformed by the model pose. Every resulting
UV curve retains its source domain and receives a continuous absolute model
error certificate, including results from the legacy sampled API. The measured
252-case maximum UV error is about `2.22e-16`, and maximum normalized model
error about `3.08e-15`; assertion limits remain `2e-8` and `1e-7` respectively.
Translated-chart cases check legitimate picks at `u=1e-7` with chart origins
`2^20` widths from zero. These points previously snapped to the boundary.

Additional checks cover 36 one-sided span/coarse-domain cases and 84 absolute
error/fixed-endpoint admission cases, bringing the total to 452. They retain
C0 kinks and source domains whose endpoints are adjacent floats. Certified
pullback accepts half-tolerance normal offsets and rejects eight-tolerance
offsets and incorrect fixed endpoints. Inputs remain unchanged.

Command cases verify flat-curve samples, native source domains, border closure,
edge midpoints, unchanged local trim domains, source retention and exact object
replay through Undo/Redo. Before the correction, 16 command cases falsely
reported a distant input and 16 failed while concatenating border parameters.

The [validation record](pullback-unit-validation.json) retains complete logs,
baseline test sources and exact source hashes. The existing adversarial test
that separates sampled acceptance from continuous certification remains in the
suite. No new Rhino capture or performance measurement was made. Arbitrary
scales, singular/noninjective surfaces, general trim topology and full native
CreateUVCrv sizing/projection parity remain outside this matrix.

```sh
cargo test -p viboceros-geometry --test surface_pullback_units
cargo test -p viboceros-command --test create_uv_curve_units
cargo test --release -p viboceros-geometry -p viboceros-command --features viboceros-command/native-smlib
cargo test --release --bin viboceros
```
