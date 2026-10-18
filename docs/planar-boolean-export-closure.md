# Planar Boolean export closure

Large planar Union and Difference could reject valid circular contours with
`InvalidPlanarFaceBoundary`. The shared contour builder checked analytic arc
endpoints, then converted those arcs into rational NURBS. Conversion uses a
different floating-point frame and could reopen a seam near coordinate zero.
At model scale `2^20`, the rejected circle and disk-strip contours had endpoint
gaps of approximately `5.43e-10` and `1.12e-9`, respectively. They were planar,
but no longer closed under the fixed curve-coincidence predicate.

The contour builder now prepares clamped NURBS segments before checking their
junctions. It checks both the original analytic endpoints and the actual NURBS
endpoints against the existing model tolerance. When reconciliation is needed,
the adjacent endpoint controls share their midpoint. Other controls, weights,
degrees and knots are retained. General curve closure and polycurve admission
keep their fixed coincidence rules. Conversion gaps larger than the supplied
tolerance remain errors.

The independent kernel regression covers 72 combinations: circle-circle and
disk-strip boundaries, both operand orders, all three operations, model scales
`2^-20`, `1`, `2^20`, and either an XY plane or a tilted, translated plane. The
original implementation rejects five combinations; the repaired implementation
accepts all 72. Normalized areas agree with analytic circle-lens and disk-strip
formulas to a maximum absolute error of about `4.98e-14` (asserted bound `1e-6`).
It also checks component counts, manifold open topology, the supporting plane
and unchanged inputs.

A focused arc-and-chord regression starts with an analytically closed contour
whose NURBS conversion introduces a gap. It checks the exported curve closes,
displacement from the unedited NURBS conversion stays within the supplied
tolerance at the same parameters, source segments stay
unchanged, and a tighter tolerance rejects the conversion gap. The previous
explicit endpoint-gap rejection regression is retained.

The existing command and application replays continue to use the retained Rhino
observations, including scaled curve discrepancies recorded as diagnostics.
Their comparison bounds and document/history assertions are unchanged. This
repair establishes export closure for the stated paths; general spline
Booleans, near-contact classification and arbitrary scales remain outside this
qualification. In particular, normalizing a unit direction with a document
distance tolerance of one or greater exposed a separate frame-construction
issue at much larger scales. The later [unit-direction repair](unit-directions-and-model-tolerance.md)
addresses the listed paths and extends the Boolean matrix through `2^80`.

The [validation record](planar-export-closure-validation.json) retains baseline
and candidate logs, exact source hashes and completed regression results.

```sh
cargo test -p viboceros-geometry --test planar_boolean_scale -- --nocapture
cargo test --release -p viboceros-geometry -p viboceros-command --features viboceros-command/native-smlib
cargo test --release --bin viboceros planar_boolean
```
