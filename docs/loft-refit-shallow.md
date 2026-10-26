# Explicit shallow Refit corners

[Angle API](loft-refit-angles.md) · [Validation](loft-refit-shallow-validation.json)

Loft Refit now preserves certified straight-region corners below one degree
when a positive explicit angle requests them. Omitted and zero angles keep the
one-degree default. The public multi-profile Rhino measurements show a separate
`0.002`-radian corner resolution floor, about `0.1146` degrees; this differs from
the `0.1`-degree constant defined by OpenNURBS. Larger requested angles retain
their own suppression boundary.

The same effective angle selects polygon corners, proposes common region charts
and rejects smooth proposals that would erase resolved tangent breaks. Unequal
profiles and line companions therefore share the appropriate corner parameters.
The existing exact straight-region extraction and complete cubic rounding bound
remain authoritative. Nonlinear positive rational clocks do not change the
certified source image. Source definitions are preserved.

Six new native families contain 157 records, including boundary holdouts,
world scaling, polynomial and rational definitions, unequal regions and a spatial
frame, all five styles, point ends and a closed radial loft. Coarse initial
cubics also smooth larger corners when the requested angle suppresses them and
the complete half-tolerance certificate succeeds. Together with ten corrected
probe records, the Rust regressions compare
complete surface definitions, domains, controls, weights and 17-by-17 samples at
`1e-9`: 77 records match on both backends, while 90 remain unsupported.
Suppressed/adaptive cases remain explicit failures; general adaptive
joining is unfinished. Rounded degree elevation can also produce slightly curved
spans that the exact straight-region path declines.

Rhino's exact `0.002`-radian edge changes branch between two model scales.
The existing conservative angular roundoff guard declines that ambiguous edge,
including one native seven-control result. Cases on either side retain their
measured behavior. No fitting-distance tolerance or positional certificate was
relaxed.

```sh
cargo test -p viboceros-geometry --lib loft --frozen
cargo test -p viboceros-oracle --test loft_refit_shallow_native --frozen
cargo test -p viboceros-oracle --features native-smlib \
  --test loft_refit_shallow_native --frozen
python3 -m unittest tools.rhino_oracle.test_loft_refit_shallow_angles
```
