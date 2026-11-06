# Point-grid input

Start `SrfPtGrid` or `SrfControlPtGrid`, enter the U and V counts, then pick or
type each point. V changes fastest: finish the first row before starting the
next. Complete coordinate scripts still work as before.

```text
SrfControlPtGrid
Degree=1
2
Degree=1
3
0,0,0
0,2,1
0,5,0
4,0,0
4,2,3
4,5,2
```

Each command remembers its own U/V counts and degrees, including accepted option
changes before cancellation. Enter reuses the current count or degree. New
instances begin with three points/degree two for `SrfPtGrid`, and four points/
degree three for `SrfControlPtGrid`. Closure and `KeepPoints` reset each run.
Use `DegreeU`/`DegreeV` and `ClosedU`/`ClosedV` for through grids, or `Degree`
at each count prompt for control grids. `KeepPoints=Yes` adds one ordered cloud.

Point entry uses the shared construction-plane, snapping, coordinate-filter and
constraint pipeline. `Undo` removes the last draft point. Enter at a point
prompt or Escape cancels. The preview joins adjacent grid points; the surface
appears after the last required point. No model objects or history entries are
created by an incomplete draft. Completion adds the surface and optional cloud
in one Undo step. Document/history or tolerance changes invalidate the draft.

The [validation record](../point-grid-input-validation.json) covers 24 completed
native workflows in both builds, including 18 constructions and 3DM round trips
per build. It compares every face's NURBS coefficients and a 9×9 parameter grid,
face sense, topology counts, trim types, attributes, cloud order and history at
`1e-6`. It retains one earlier driver timeout as a diagnostic. This is not a
continuous distance certificate or full trim-index/domain comparison. Arbitrary
mouse trajectories, persistence across application restarts and comparative
speed remain unqualified; [solver limits](../point-grid-surfaces.md) still apply.

```sh
cargo test --bin viboceros app::tests::surface_point_grid
python3 -m unittest tools.rhino_oracle.test_point_grid_input
```
