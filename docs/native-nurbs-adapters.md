# Native NURBS adapters

[Native modeling](native-modeling.md) · [Validation](native-nurbs-adapters-validation.json)

The usd-brep/SMLib adapter keeps its existing public functions and numerical
policies. Its implementation is split by responsibility:

| Module under `crates/viboceros-smlib/src/native/knot_refinement/` | Responsibility |
| --- | --- |
| `loft.rs` | Loft and Sweep section preparation and construction |
| `interpolation.rs` | Shared collocation, rebuild, fit, matching and tweens |
| `interpolation/point_grid.rs` | Open and periodic point-grid collocation |
| `interpolation/tests.rs` | Independent solver and tiny-channel regressions |
| `trim.rs` | Isocurves, trimming, splitting, seams and Bezier extraction |

The parent `knot_refinement.rs` retains the FFI declarations, NURBS payload,
clamping, knot editing and degree changes. Public reexports keep callers and
native operation hooks unchanged. The validation record checks the original
81 function bodies and shared ABI/payload definitions against their immutable
Git reference, alongside native integration tests.

The same investigation retains fresh
[scalar-clock counterexamples](loft-fitting-clock.md) and a complete positional
bound for a reduced fitted-piece join. The join differs from Rhino's controls
by about `6.7e-7`, while a continuous bound places the entire profile within
`5.6e-7` of the captured native profile. Its source-profile bound is below
`0.0005` for a requested fit tolerance of `0.001`. These are geometric
certificates, not an implementation of Rhino's adaptive fitting policy.

```sh
cargo test -p viboceros-smlib --features native --lib knot_refinement
cargo test -p viboceros-oracle --test refit_join_certificate
```
