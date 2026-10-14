# Native surface-normal curve pulling

The native adapter exports `pull_curve_to_surface(source, surface, tolerance)`.
It uses usd-brep's public `SmSurface::DropAndTrimCurve` and `LiftCurve` operations,
returning paired spatial/UV NURBS curves over the finite natural surface domain.
This supplies the geometry producer for the [Pull command](commands/pull.md).
The command adds source/target picking, points, face trim clipping and spatial
seam admission. Mesh curves and Loose remain pending.

The high-level `SmApiProjectCurve` and `SmApiDropCurveToSrf` helpers fix their
approximation tolerances internally. The adapter instead passes one eighth of
the normalized model tolerance to both native operations, checks their reported
normal/lift deviations, restores original geometry and surface UV coordinates,
and proves each complete spatial image lies within model tolerance of
`surface(UV(t))` using the existing continuous surface-curve certificate. Native
failures and inconclusive certificates decline; inputs are never edited.
This certifies surface membership, while global nearest-branch choice still
relies on the native solver and remains independently unqualified.

Preparation reuses the shared translated/scaled coordinate and unit-domain
adapters. Their degree, weight-range, repeated-knot and representability limits
apply. The source curve limit is degree 17; target surfaces are limited to degree
16 by certificate support. Returned spatial/UV pairs share the native curve
parameterization; original source parameter domains are not promised. Two UV
preimages of a cylinder seam can yield coincident spatial witnesses. They remain
available to trim-aware command admission. Broader curved families, singularities,
closed-seam parity and comparative performance remain open.

The pinned kernel used `SM_SO_INTERSECT` to seed interval starts even when a
curve was explicitly marked off the surface. At tight tolerances, a line one
unit above a plane returned success with no curves when crossing its finite
boundary. The [two-site repair](../third_party/patches/usd-brep-surface-drop-start-normal.patch)
uses `SM_SO_NORMALIZE` for those starts, retaining intersection seeds for curves
already on the surface. CMake patches a generated copy; the submodule stays
unchanged. A baseline executable overrides only that class's archive object,
keeping the rest of the build equal. Twelve direct recipes retain both
[baseline](smlib-surface-pull-baseline.jsonl) and
[repaired](smlib-surface-pull-repaired.jsonl) output. Three off-surface boundary
cases change from empty to correctly clipped intervals; the other nine remain
unchanged.

The Python oracle operation `pull_curve_surface` takes full NURBS `curve` and
`surface` definitions. Viboceros requires `--native-smlib`; Rhino uses public
[Curve.PullToBrepFace](https://developer.rhino3d.com/api/rhinocommon/rhino.geometry.curve/pulltobrepface).
It returns arc-length samples, length and closedness, preserving raw native seam
witnesses. Closed seams are not canonicalized. Six retained native recipes cover
planar interiors, boundary clipping, misses, axial and closed cylinder pulls,
and translated geometry with unequal UV domains. Fresh Rhino capture remains
blocked by the [Wine SSPI diagnostic](oracle-sspi.md), so these are local analytic
checks rather than a complete compatibility result. Ownership regressions cover
source, surface, face and result cleanup at SDK failure boundaries.

```sh
cargo test -p viboceros-smlib --features native --lib surface_pull
cargo test -p viboceros-oracle --features native-smlib --lib surface_pull
python3 -m tools.rhino_oracle viboceros \
  tools/rhino_oracle/fixtures/pull_curve_surface.json --native-smlib
python3 -m unittest tools.rhino_oracle.test_surface_pull
```

To reproduce the direct kernel probes, build the opt-in
`viboceros_smlib_surface_pull_probe` and `viboceros_smlib_surface_pull_baseline`
targets in the native CMake build directory printed by Cargo's build script.
See [qualification](smlib-surface-pull-qualification.json),
[integration](usd-brep.md) and [kernel issue ledger](usd-brep-limitations.md).
