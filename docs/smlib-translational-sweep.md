# Native fixed-orientation curve-path extrusion

Linux builds with `--features native-smlib` use usd-brep for
`ExtrudeCrvAlongCrv` sheets and capped solids. The separate command module
keeps parsing, path selection and transactional document policy. Native builds
exclude the direct Rust tensor and cap builders from this command.

The adapter calls existing `SmBSplineSurface::CreateSweepSurface` with
`bDoTranslationalSweep=True`. It copies the path and translates that private
copy by its evaluated start point using native `Transform`. This constructs
`profile(u) + path(v) - path(start)` exactly, with fixed profile orientation,
profile U and path V. The public `SmApiCreateSweep` wrapper instead selects a
rotating approximate sweep; its source-audited restriction is in the
[limitations ledger](usd-brep-limitations.md).

Closed planar profiles along open paths use native `CreateEndCapsFromSurface`
on a private B-rep configured with the document tolerance. Cap construction,
checked stitching, manifold checks and exact NURBS conversion now share the
apex extrusion helper. Export reuses the existing validated B-rep transfer.
Caps whose endpoint displacement remains in the profile plane are rejected.
Open/nonplanar profiles and closed paths retain sheet output when Solid is
requested. Constant paths are rejected before native tensor construction.

The native path is imported once per command batch. Each profile is built
relative to its start point and restored with the existing affine transform.
Native input curves remain owned and usable. Profile and path preparation
reuse [native active-interval clamping](smlib-curve-import.md) and uniform negative-gauge handling; mixed-sign
weights and discontinuous interior knots retain importer restrictions. Native
construction checks the existing one-million-control transfer limit before
allocating the tensor product.

All results are staged before document edits. Output attributes remain on the
current layer, source profiles obey DeleteInput, the path is retained and
deselected, and source groups, selection order and Undo/Redo follow the existing
command flow. A later coplanar cap failure preserves the batch and prior redo.
Separate tangent-splitting behavior remains further work: the existing command
accepts SplitAtTangents but does not split these tensors.

The [live fixture](../tools/rhino_oracle/fixtures/curve_native_along.json) covers
a line, a rational profile, a kinked polyline, a reversed path, a closed circular
path, unclamped profiles/paths and uniformly negative homogeneous gauges.
The worker disables CreaseSplitting and SplitAtTangents and records complete
raw NURBS sheet definitions. See the
[official command options](https://docs.mcneel.com/rhino/8/help/en-us/commands/extrudecrvalongcrv.htm).
The [raw observations](../tools/rhino_oracle/observations/curve_native_along.json)
and [comparison](smlib-translational-sweep-comparison.json) retain six complete
raw matches out of seven, without normalization. The unclamped-path case has
different outer knots and controls because canonical import clamps its active
interval. All seven captured 9×9 point grids match at `1e-10` absolute / `1e-12`
relative epsilon; maximum point-coordinate error is `7.11e-15`. Grid agreement
does not certify the continuous surface. The same rebuilt probe also matches
all six retained straight-extrusion/apex sheet records after shared-helper cleanup.
Native cap regressions independently check rational translation grids, preserved
parameters and input handles, analytic signed volume for positive/negative paths,
and invalid paths/caps. These cases do not establish arbitrary self-intersection,
near-planarity, scale extremes, tangent splitting or Rhino performance parity.

Release validation passed 1,310 native command tests and 59 native bridge tests
(three tests ignored across those suites), two portable command regressions and
1,095 Python oracle tests. Native clippy with warnings denied and workspace
formatting checks passed.

```sh
CMAKE_BUILD_PARALLEL_LEVEL=10 cargo test --release -p viboceros-smlib \
  --features native --test translational_sweep
CMAKE_BUILD_PARALLEL_LEVEL=10 cargo test --release -p viboceros-command \
  --features native-smlib extrude_curve_along_curve
```
