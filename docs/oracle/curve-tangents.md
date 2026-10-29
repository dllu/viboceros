# Ordered curve tangent queries

The Python oracle operation `nurbs_curve_tangents` queries unit directions in
native parameter coordinates. Query order and duplicates are retained; an
interior knot uses its right span and the final endpoint uses its left span.
An unavailable native tangent is `null`, including the captured third-order
stationary cases. This can differ from a mathematically defined limiting tangent.

```python
operation = {
    "op": "nurbs_curve_tangents", "id": "directions",
    "curve": nurbs_definition, "parameters": [1.0, 0.0, 0.5, 1.0],
}
```

Results contain `before`, `after`, `parameters` and `tangents`. The constructed
source is preserved. An explicit curve `domain` changes the parameter range;
queries outside that range fail before source construction. Limits are degree
16, 512 controls and 4096 queries per operation. Integer JSON parameters become
floating values. Rhino's homogeneous control storage can round a constructed
Euclidean point when nonunit weights are supplied.

Both Viboceros backends reuse the existing owned OpenNURBS batch query.
[Validation](curve-tangents-validation.json) covers 72 queries in 18 Rhino cases,
including three unavailable tangents, hard corners, weight gauges and domains.
The fitting diagnostic also clamps backward handle cosines to zero, matching
the qualified proposal formula. Its 21 new public-frame captures do not resolve
the existing Refit cutoff discrepancy. Substituting native endpoint evaluation
did not improve that comparison and was reverted; storage flags did not change
the captured candidate directions.

```sh
cargo test --frozen -p viboceros-oracle --test curve_tangents_native
cargo test --frozen -p viboceros-oracle --features native-smlib --test curve_tangents_native
python3 -m unittest tools.rhino_oracle.test_curve_tangents
```
