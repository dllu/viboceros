# Loft section rebuilding

[Loft](loft.md) · [Curve Rebuild](commands/rebuild-curves.md)

`Loft Rebuild=6` rebuilds the profiles before lofting, retaining source objects
and the usual layer, selection and undo policies. `RebuildCount` and
`RebuildPointCount` are aliases. `Simplify=Rebuild RebuildCount=6` selects the
same operation; `Simplify=None` disables it. Interactive Loft accepts these
options and value prompts alongside picked point ends.

The ordered geometry API is `try_loft_nurbs_curves_rebuilt`; native builds expose
`viboceros_smlib::loft_surface_rebuilt`. They reuse the existing arc-length
Greville interpolation, conditioning and portable/native solvers. Loft uses
clamped profiles, including closed profiles with a repeated seam control;
ordinary Curve Rebuild retains its periodic policy. Tangents are not projected.
Degrees are `min(3, count - 1)`. Matching the public SDK, 0 and 1 disable rebuilding;
closed profiles also retain their original basis for count 2. Larger closed
requests add one seam control. The 512-control limit includes that extra control
and is checked before fitting any profile. [Refit remains unimplemented](loft-refit.md);
its native fitting and parameterization differences are retained for investigation.

Fresh Rhino 8.32 captures cover 433 ordered API cases: 397 returned surfaces
match both backends within absolute `1e-6` across full control nets, weights,
knots, domains and 289 samples. The maximum full-record difference is about
`3.60e-7`. The other 36 records are Uniform lofts with point ends, which both
engines reject. All API source definitions remain unchanged.

This rebuild is an approximation. Exact circle Greville witnesses and independent
native Curve Rebuild outputs expose station differences above `1e-9`; the
stricter comparisons remain explicit ignored diagnostics. Existing unreconstructed
Loft tests retain their own tighter tolerances. No continuous error bound or
arbitrary-scale Rhino agreement is established.

A separate 140-command capture originally matched 105 complete raw records on
the portable backend and 106 on the native backend at `1e-6`. The later
[profile-ordering change](loft-ordering.md) resolves the open cubic U-chart
differences. With [adaptive crease checks](surface-creases.md), current totals are
125/126 matches, with 15/14 closed-solid face-sense differences remaining.
The complete raw comparison remains an explicit ignored diagnostic. All 60 open
records additionally run as ordinary regressions at `1e-6`.

The [original rebuild record](loft-rebuild-validation.json) retains the initial
measurements; the [ordering record](loft-ordering-validation.json) records the
ordering-stage comparison; the [crease record](surface-creases-validation.json)
records the subsequent partitioning improvement.

```sh
cargo test -p viboceros-geometry --test loft_rebuild_native
cargo test -p viboceros-smlib --features native --test loft_rebuild_native
cargo test -p viboceros-oracle --test loft_rebuild_native
# Retained strict numerical and full command diagnostics:
cargo test -p viboceros-oracle --test loft_rebuild_native -- --ignored
```
