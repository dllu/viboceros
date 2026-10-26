# Loft Refit diagnostic angles

[Adaptive pieces](loft-refit-adaptive.md) · [Refit preparation](loft-refit-preparation.md)

Independent fitting diagnostics now inherit the explicit Loft Refit angle, or
one degree for the overload without an angle. Previously they used the document
angle, which is often `1e-8` radians in oracle requests. New observations record
`refit_probe_angle_tolerance`. Clock, profile-set, frame, source, Bezier and
standalone-fit inspections all receive that value. Ordinary captures are unchanged.
McNeel documents [the one-degree SDK default](https://developer.rhino3d.com/api/rhinocommon/rhino.rhinomath/defaultangletolerance)
and [the two Loft Refit overloads](https://developer.rhino3d.com/api/rhinocommon/rhino.geometry.brep/createfromloftrefit).

Ten native records check implicit and explicit one-degree angles, two smaller
explicit angles, and four shallow-bend sources across scale. Their complete Loft
surfaces retain the earlier observations. Paired coincident public preparation
reproduces each first Loft profile's controls and weights exactly. One input
still produces different definitions at the same angle. C0 region matching can
change parameter charts, so equal controls do not establish global parameter parity.
Two earlier comparisons used the wrong inspection angle and returned seven
controls instead of the corresponding eight or ten.

Four separate public single-knot removals, at two knot parameters and two model
scales, match both portable and SMLib-backed basis interpolation within `1e-9`.
The Rust regressions also compare 65 evaluation stations and retain the inputs.
This rules out that single-knot operation as the explanation for the remaining
Loft joining differences; the final composite fitting policy remains unfinished.
The smaller explicit angles also retain native quarter-degree polygon corners
that the current Rust one-degree polygon floor still declines.
Earlier captures and their archived producers are immutable. Their independent
inspection angles must be read from their captured document settings.

```sh
python3 -m unittest tools.rhino_oracle.test_loft_refit_probe_angles \
  tools.rhino_oracle.test_loft_refit_probe_captures
cargo test -p viboceros-geometry --test refit_join_knots_native --frozen
cargo test -p viboceros-smlib --features native --test refit_join_knots --frozen
```
