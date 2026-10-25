# Fixed-orientation curve-path extrusion

`ExtrudeCrvAlongCrv [PathName=name] [Output=Surface] [Solid=Yes|No]
[DeleteInput=Yes|No] [SplitAtTangents=Yes|No]` translates each selected profile
along the path without rotating it. Name the path or select it last. Outputs
use the current layer and default attributes; the path is retained and deselected.
Profiles remain selected unless deleted. Defaults are Solid=No, DeleteInput=No,
and SplitAtTangents=No.

`SplitAtTangents=Yes` now partitions both curve directions at geometric G2
breaks using a one-degree angular tolerance. `CreaseSplitting Enable|Disable|Toggle`
independently controls G1 cuts above 0.1 degree, initially enabled. These two
policies are combined; repeated knots or unequal parameter speeds alone do not
force edges. The new application preference is currently connected to this
command; integration with other surface constructors remains work to do.

`Solid=Yes` caps planar boundary loops of the resulting surface. This also
handles open profiles along closed paths: a bent open profile can yield planar
rim loops and a capped solid. A flat sheet remains a sheet. Requesting caps on
an already closed result is rejected. All results are staged before editing;
failed commands preserve sources, groups, selection, and prior redo history.

Native builds use usd-brep `SmBSplineSurface::CreateSweepSurface` with
`bDoTranslationalSweep=True`. A private path is shifted by its evaluated start;
profiles are recentered independently and restored afterward. This constructs
`profile(u) + path(v) - path(start)`, retaining exact rational tensor data.
The public `SmApiCreateSweep` wrapper selects a rotating approximate sweep;
see the [limitations ledger](usd-brep-limitations.md). Portable builds use the
existing Rust tensor constructor. Both routes reuse public OpenNURBS continuity
queries, the shared exact face grid, and existing planar-hole capping. Closed
seams are retained; cap and wall boundaries share their original edges.

The native path is imported once per batch. Active-interval clamping and
uniform negative-gauge handling use the [shared importer](smlib-curve-import.md).
Its mixed-sign weight and positional-discontinuity restrictions remain.
Existing tensor, face-grid, and capping resource limits apply.

The [qualification record](curve-path-extrusion-qualification.json) binds 272
licensed command captures: 168 profile/path combinations and 104 angular and
curvature cases. API and ordinary application-command replays cover face counts,
solid classification, bounds, volume, retained source identity, and successful
Undo/Redo at `1e-6` absolute epsilon. Native helper selection and attributes,
interactive picking, continuous surface distance, and performance are not qualified.
Eight native commands failed; in four, Rhino had already deleted the profile.
Viboceros rolls back those deletions, so failure-state parity is deliberately
incomplete. Original producer sources are archived with each capture.

The older [seven-case fixture](../tools/rhino_oracle/fixtures/curve_native_along.json)
and [raw comparison](smlib-translational-sweep-comparison.json) remain separate:
both splitting policies were disabled. Six raw NURBS definitions match exactly;
the unclamped path has different outer knots and controls after canonical import.
All seven captured 9×9 grids match at `1e-10` absolute / `1e-12` relative epsilon,
with maximum coordinate error `7.11e-15`. Grids do not certify the continuous locus.

```sh
cargo test -p viboceros-oracle --features native-smlib --test curve_path_native
cargo test -p viboceros-command --features native-smlib --lib extrude_curve_along_curve
```

See the [official extrusion options](https://docs.mcneel.com/rhino/8/help/en-us/commands/extrudecrvalongcrv.htm)
and [crease setting](https://docs.mcneel.com/rhino/8/help/en-us/options/general.htm#CreaseSplitting).
