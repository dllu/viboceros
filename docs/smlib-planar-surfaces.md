# Native planar surfaces

Linux builds with `--features native-smlib` now use usd-brep to construct
PlanarSrf and PlanarSurface regions. Parsing and document staging live in the
separate `planar_surface` module. The previous area-sorted containment forest,
projected trim construction and sibling-hole rebuilding are compiled into the
command adapter only in portable builds.

Native plane grouping uses the existing `AreCoPlanar` operation on each entire
candidate group. Region construction and even/odd nesting use
`CreatePlanarFacesWith3DCurves` directly. No new intersection or containment
algorithm was implemented. Native-produced faces use the shared NURBS/B-rep
exporter and Rust validation before document changes.

The live overlapping-hole fixture corrected the older portable policy. Rhino
and the native region constructor both create an area-83 outer face with two
holes, plus an independent area-16 face. The portable adapter detaches both
competing holes and creates three faces totaling area 131; that remaining
compatibility difference is retained explicitly in the tests. A provisional
contact-detachment adapter was removed after the capture: the kernel already
provides the measured geometry.

The owned receiving B-rep gets the document tolerance before construction.
The high-level planar wrapper's numeric tolerance argument is ignored by the
kernel, so the bridge configures the receiving B-rep explicitly before using the existing
kernel entry point.
Curve copies are consumed on success. Partial failure detaches copied input
curves from native edges before cleanup, matching the existing SM_API ownership
contract; original native handles remain usable.

Existing modeling-profile preparation handles unclamped active intervals and
uniformly negative gauges. Closed planar sources participate; open/nonplanar
sources retain the existing skip policy. Faces are staged before any edits.
Current-layer output attributes, valid-source deletion, groups, selection and
Undo/Redo retain the shared command path. A later rejected profile preserves
the entire model, group membership, selection and the previous redo entry.

Regressions cover nested holes/islands, independent overlapping siblings,
multiple planes, rational circles, current-layer metadata, source preservation,
failed native creation, and plane separation near the model tolerance. Polygon area differs by ordinary integration roundoff in one
case (`99.00000000000006` versus `99`), within the retained `1e-12` test bound.
This does not qualify arbitrary self-intersections, tangencies, topology, scale
extremes or Rhino performance parity. Native calls are limited to 128 profiles.

The [live fixture](../tools/rhino_oracle/fixtures/planar_surface_native.json)
measures region counts, loops, area, bounds and source retention. Its deletion
cases perform explicit post-command deletion in Rhino: DeleteInput is an
application extension, not a Rhino PlanarSrf option. The
[official command description](https://docs.mcneel.com/rhino/8/help/en-us/commands/planarsrf.htm)
documents independent overlapping curves and contained hole boundaries.
Region records use chronological object creation (Rhino RuntimeSerialNumber),
with no geometry-based sorting. Native records query usd-brep's tight trimmed
bounds: Rust's ordinary B-rep bounds are deliberately conservative and include
plane support outside the trimmed region. The probe's `native-smlib` feature
selects the native command adapter and this native bounds query together.
See the [comparison](smlib-planar-comparison.json) and
[limitations ledger](usd-brep-limitations.md) for qualification.

All six chronological region records match at `1e-10` absolute / `1e-12`
relative epsilon. These are region properties, not complete B-rep definitions.
Release validation passed 1,308 native command tests and 52 native bridge tests,
with three tests ignored across those suites. The portable planar regressions
and 1,095 Python oracle tests passed; native clippy and formatting checks passed.

```sh
CMAKE_BUILD_PARALLEL_LEVEL=10 cargo test --release -p viboceros-smlib \
  --features native --test planar_faces
CMAKE_BUILD_PARALLEL_LEVEL=10 cargo test --release -p viboceros-command \
  --features native-smlib planar_surface
```
