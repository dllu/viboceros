# Bounded surface knot reduction

[Geometry guides](README.md) · [Curve reduction](knot-reduction.md) · [Surface bounds](surface-correspondence.md)

`NurbsSurface::try_reduce_knots_in_range(direction, minimum, maximum, limit)`
reduces eligible knot groups in U, V, or both directions. Groups are selected
by exclusive multiplicity bounds and reduced to simple knots; degree-one
interior knots disappear. U groups run in descending order, followed by V.
`try_reduce_knot_multiplicity_with_bound(direction, knot, target, limit)`
also supports a requested final multiplicity at one exact interior knot.

Every accepted proposal is checked against the complete original tensor,
including its interior and one-sided knot limits. Errors cannot accumulate
between groups or directions. The returned `SurfaceKnotReduction` owns the
surface, total removed knot count and complete displacement bound. `Some(0.)`
requires exact agreement; `None` explicitly permits unrestricted changes.
Outside or inconclusive proposals are declined. Inputs remain unchanged on
failure, including a failure after an earlier successful direction.

The portable fitter shares one collocation factorization across all control
rows. Constant coordinate and weight channels are restored before testing the
bound. `viboceros_smlib::reduce_surface_knots_in_range` shares selection and
certification while fitting through usd-brep. Endpoint tangents, trimming and
topology are not certified.

Selected periodic directions are unsupported. Bounded inputs require degrees
at most 16, at most 4096 controls and nonzero common-sign weights. At most 256
eligible groups are allowed across the selected directions; each basis fit is
limited to 64 million cubic work units. All inputs are capped at one million
controls. The surface certificate's arithmetic and subdivision limits apply.

The JSON/Python operation `surface_knot_reduction` accepts a `surface`
definition, `direction` (`u`, `v`, or default `both`), optional
`minimum_multiplicity` (default one), `maximum_multiplicity` (default the larger
degree plus one), and `absolute_tolerance` (default request tolerance).
It returns the edited definition, removed count, an 81-point sample grid, and
unchanged before/after source records. This SDK comparison protocol maps zero
to unrestricted removal, following the curve probe convention. The Rust kernel
keeps zero strict.

Native comparisons retain cases declined by the stronger normalized parameter
bound separately from matching results. Failure to certify that correspondence
does not prove that two geometric images differ. All replayed outputs also
undergo a complete surface comparison after a 3DM round-trip.

The [validation record](../surface-knot-reduction-validation.json) owns 88
Rhino cases: 64 numerical matches and 24 explicit declines on each backend.
It retains both failed capture attempts, checks source ownership and includes
the 119-case averaging regression. SDK range semantics are documented in the
[Rhino API reference](https://developer.rhino3d.com/api/rhinocommon/type/rhino.geometry.collections.nurbssurfaceknotlist/).

```sh
cargo test -p viboceros-geometry --lib knot_reduction
cargo test -p viboceros-oracle --test surface_knot_reduction_native
cargo test -p viboceros-oracle --features native-smlib --test surface_knot_reduction_native
python3 -m tools.rhino_oracle.report_surface_knot_reduction
```
