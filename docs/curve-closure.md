# Curve closure and shared seam grips

`Polyline3::is_closed` follows the public OpenNURBS predicate: at least four
vertices, coincident endpoints, and an interior vertex distinct from both ends.
Coincidence compares each coordinate using the fixed absolute threshold
`2^-32` or the relative threshold `(|a| + |b|) * 2^-42`. Model tolerance does
not change this classification. NURBS and composites retain their own native
interior-sample rules; equivalent representations need not classify identically
when they are numerically collapsed.

A closure query preserves all vertices, controls and parameters. `SelClosedCrv`
and `SelOpenCrv` use that state. `CloseCrv` leaves already-closed sources unchanged,
including numerically distinct endpoints and redo history. Parameterization and
3dm roundtrips retain those coordinates.

`PointsOn` and `ExtractPt` share one grip at a closed seam. A real control-grip
edit rebuilds seam aliases at the displayed position, even if a different grip
moves. Zero-distance moves keep the original coordinates and representation.
Degree-one NURBS with constant weights become polylines after an edit when their
knots support that representation. Composite edits rebuild NURBS leaves in the
original parent intervals, sharing junction grips while retaining independent
nonadjacent grips at identical coordinates.

The retained matrix has 163 source recipes: 150 valid document objects, five
invalid polyline prototypes queried only through the SDK, and eight composite
construction rejections caused by invalid native line segments. It includes
absolute and relative threshold boundaries, axis-wise diagonal gaps, tiny and
large coordinates, custom domains, weighted and unit-weight rational NURBS,
degrees two and three, flat and nested composites, and nontransitive coincidence.
Native commands cover two selection queries, 44 `ExtractPt` calls, 28 grip moves
including no-ops, and `CloseCrv` with an open witness. The witness lets the native
command finish after it filters out already-closed preselection; retained source
geometry is compared separately.

Tests compare closure exactly, grip/extraction locations at `1e-8` absolute
epsilon, edited geometry and representation, source preservation, logical layer
membership through 3dm, and Move Undo/Redo. Layer IDs belong to each document;
roundtrips compare layer names. Invalid prototypes are not inserted into the
retained 3dm. The kernel can represent finer segments than native polyline-curve
validation permits; this work does not equate constructor validity policies.
These bounded checks do not qualify every curve family, composite grip edit,
near-circular conic, arbitrary modelling operation, or performance characteristic.

[Validation results and source receipts](polyline-closure-validation.json)

```sh
cargo test -p viboceros-geometry --test polyline_closure --test polycurve_grips
cargo test -p viboceros-command --test polyline_closure
cargo test -p viboceros app::tests::polyline_closure
python3 -m unittest tools.rhino_oracle.test_polyline_closure_probe
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.generate_polyline_closure
```

The closure policy comes from public
[OpenNURBS polyline code](https://github.com/mcneel/opennurbs/blob/23fc677ba06e49212296ca75fab7fb6c2851b4ce/opennurbs_polyline.cpp#L124)
and the existing shared kernel point-coincidence predicate. Native validation
also rejects adjacent point-coincident vertices; the corresponding prototypes
remain in the observation data as invalid SDK inputs.
