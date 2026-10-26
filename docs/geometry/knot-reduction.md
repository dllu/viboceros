# Bounded NURBS knot reduction

[Geometry guides](README.md) · [Cubic approximation](../cubic-approximation.md)

`NurbsCurve::try_reduce_knot_multiplicity_with_bound(knot, target, limit)`
proposes a lower multiplicity at an exact interior knot. It returns a curve,
removed count and continuous positional bound, or `None` when the proposal cannot
be certified. Reducing a cubic join from multiplicity three to two can retain
its C1 basis without forcing the C2 basis of a simple knot.

`try_reduce_knots_in_range(minimum, maximum, limit)` reduces eligible groups in
descending knot order. Multiplicity eligibility is exclusive, with minimum zero
treated like one for degrees above one. An eligible group collapses to a simple
knot; degree-one interior knots disappear. The minimum selects groups, rather
than specifying their resulting multiplicity.

Both APIs use existing basis interpolation and the complete original-curve
Bernstein certificate. Each accepted group is checked against the original
source, preventing displacement from accumulating across edits. Constant stored
coordinate and weight channels are restored before certification to avoid
unnecessary interpolation roundoff. The source is unchanged. Bounds certify
position in normalized parameter correspondence, not tangent preservation or
topology. Periodic inputs are unsupported; bounded edits require degree at most
16 and nonzero common-sign weights. More than 256 eligible groups fail before
editing. Existing certificate arithmetic and work ceilings also apply.

The range API accepts `Some(0.)` for strict zero error and `None` for explicitly
unrestricted removal. The SMLib adapter `reduce_curve_knots_in_range` shares
selection and certification while using usd-brep's basis solver.

The JSON/Python operation `curve_knot_reduction` accepts `curve`, optional
`minimum_multiplicity` (default one), `maximum_multiplicity` (default degree plus
one), and `absolute_tolerance` (default request tolerance). It returns the edited
definition, removed count, 33 samples and unchanged before/after source records.
This SDK comparison protocol translates zero to unrestricted removal, matching
the captured Rhino SDK sentinel. The Rust kernel keeps zero strict.

Native captures include polynomial, rational, linear and fitted-piece joins.
Rhino's SDK removes all three corners of the captured polyline even at `1e-9`,
moving one source vertex more than three units from the resulting segment. The
bounded implementation declines those unsafe edits. The
[qualification record](../knot-reduction-validation.json) separates these cases
from numerical matches. Adaptive Loft Refit joining remains unfinished.

```sh
cargo test -p viboceros-geometry --lib knot_reduction
cargo test -p viboceros-oracle --test knot_reduction_native
cargo test -p viboceros-oracle --features native-smlib --test knot_reduction_native
```
