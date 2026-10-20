# Stretch fitting and performance

Stretch now represents eligible planar B-rep faces as ruled cubic NURBS instead
of fitting a dense tensor product. The source must be an exactly affine,
uniformly weighted, degree-one 2×2 patch. A tiny warped corner does not qualify.
The new UV chart uses distance along the stretch axis as one coordinate; the
clamped smoothstep map is then piecewise cubic in that coordinate and linear in
the other. The chart has positive orientation. Trim curves are transformed with
their degrees, knots, weights and incidence retained; shared model-space edges
are fitted once. Candidate conditioning and normal B-rep assembly checks still
apply. Unsupported or rejected candidates use the general fitter.

This changes the underlying UV domains and control net. Physical geometry and
trimmed regions agree with the point map, but raw UV parameters and subsequent
control-net edits can differ from Rhino's fitted surfaces. The kernel's
`PreserveStructure` mode retains the existing source-net path. Moving a control
grip on a reparameterized single face rebuilds its boundaries through the
existing certified surface-edit adapter, with ordinary document history.

The general surface fitter also batches evaluation of each validation grid,
reusing U contractions within a V knot span. It retains every validation station,
left/right knot limit, scalar arithmetic result and recovery path. Fitting
tolerances, refinement criteria and work limits are unchanged.

## Measured geometry time

The [receipt](stretch-performance-validation.json) retains matched input hashes,
binary hashes, timings and checks. These are release builds on the same machine,
with the optional usd-brep/SMLib backend enabled. Each row uses the same retained
Stretch fixture at factor 0.5. Rust values are medians of three calls; native
values are medians of five calls after one warm-up.

<!-- timing-table -->

| Fixture | Before (ms) | After (ms) | Rhino SDK (ms) | Before/after |
| --- | ---: | ---: | ---: | ---: |
| Reverse / Curve | 1.376 | 1.493 | 2.132 | 0.9× |
| Reverse / Surface | 29.056 | 3.846 | 6.335 | 7.6× |
| Reverse / Box | 1.494 | 0.959 | 2.660 | 1.6× |
| Spatial / Curve | 2.807 | 2.880 | 1.504 | 1.0× |
| Spatial / Surface | 4744.043 | 5.479 | 8.411 | 865.9× |
| Spatial / Box | 18322.749 | 16.410 | 51.000 | 1116.6× |
| Skew / Curve | 1.125 | 1.132 | 2.324 | 1.0× |
| Skew / Surface | 15163.648 | 13.958 | 10.881 | 1086.4× |
| Skew / Box | 26156.495 | 22.338 | 29.461 | 1170.9× |

<!-- /timing-table -->

The Rust timer covers geometry deformation, including its fitting and B-rep
assembly checks. Fixture construction, document mutation, independent oracle
validation, export and destruction of previous results occur outside the timer.
The native timer covers the public `StretchSpaceMorph.Morph` call and its Python
status conversion, excluding duplication and result readback. Rhino runs under
FEX/Wine on this machine. Its SDK fitting tolerance is `1e-6`; Viboceros retains
the command's existing `1e-5` component floor and four B-rep component budgets.
The resulting representations and approximation errors differ. These nine cases
measure this optimization, rather than establish general kernel performance.

## Geometry checks

All 119 retained command workflows pass through the command API with portable
and native interpolation, and through the interactive prompt tests. Comparison
recovers an affine UV correspondence independently from paired trim controls
before checking physical points. Existing native comparison bounds are
unchanged. Separate off-grid checks compare against the exact point map.

Kernel tests check transformed normals, holes, nonrectangular trims, shared
topology, bounding boxes and box volumes. The volume reference independently
integrates the point map's Jacobian. A nearly planar warped input must fall back;
`PreserveStructure` must retain its original basis. Grid tests compare scalar
results bit for bit at discontinuities, invalid parameters and extreme signed
weights, including early visitor failure.

Rhino reopened all nine retained Rust-exported 3dm files and reported valid
geometry; all three boxes remained solids. At the sampled stations, the six
planar-face/box exports differed from the exact map by at most `7.11e-15` model
units in the forward direction. The inverse-map checks measured at most
`1.22e-14` against the original shapes. Curves retain adaptive fitting: their
largest forward error was `6.98e-6`, with inverse error `1.34e-5` for the monotone
factor-0.5 map. These are sampled distances, not continuous Hausdorff certificates.

The public SDK producers and exact exported bytes are retained in
[the timing capture](../tools/rhino_oracle/observations/morph_fit_benchmark.json)
and [the readback capture](../tools/rhino_oracle/observations/morph_fit_readback.json).
Earlier [Stretch validation](stretch-validation.json) remains a historical
receipt for the command implementation before this optimization.

```sh
cargo run --release -p viboceros-command --features native-smlib --example profile_morph_fits -- axis_Spatial_Box 3
cargo test -p viboceros-geometry --lib brep::stretch --config 'profile.test.package.viboceros-geometry.opt-level=2'
cargo test -p viboceros-command --test stretch_command --config 'profile.test.package.viboceros-geometry.opt-level=2'
python3 -m unittest tools.rhino_oracle.test_morph_fit_performance tools.rhino_oracle.test_stretch
```
