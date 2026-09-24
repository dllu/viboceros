# Mesh snapping

[Snap controls](object-snap-controls.md) · [Near](near-snaps.md) · [Provenance and hashes](mesh-snaps-provenance.json)

Enable **Snap modes → Snap to mesh wires**, or enter
`SnapToMeshes Enable`. `Disable` and `Toggle` are also accepted; `On`/`Off`
are not. Supply the option on the same line. The switch defaults off and is
independent of the enabled feature modes, Osnap suspension, and one-shot overrides.
It neither enables Near nor consumes a pending one-shot or modeling prompt.
Like other drafting settings, it is session-local and outside document history.

Supported features are Near, Mid and straight-wire Int on actual triangle/quad face-boundary wires.
Coincident vertices are welded by exact location; shared edges appear once.
Quad display-tessellation diagonals are not snap wires, but actual shared edges
between triangle faces are. Document tolerance changes do not erase existing
short nonzero wires. Hidden objects/layers are excluded; locked sources remain
eligible.

Mesh Mid requires proximity to the midpoint itself, including Mid-only and
one-shot Mid. The retained mesh cases did **not** exhibit the
[whole-segment hover](mid-hover-snaps.md) measured on curves. On one mesh, a
captured Mid takes precedence over Near; separate objects compete by capture
distance. Mesh vertices do not become End or Point targets through this switch.
Rhino's [object snap reference](https://docs.mcneel.com/rhino/8/help/en-us/user_interface/object_snaps.htm)
lists Vertex separately from the SnapToMeshes wire modes. Vertex captures mesh
vertices with the mesh-wire switch either on or off; Point and End do not
capture them. The vertex index is built only when Vertex is requested and
shares the mesh snapshot's invalidation lifetime. Straight mesh wire Int is
described in [intersection snaps](intersection-snaps.md). Perp remains
unimplemented.

The [six-case Vertex fixture](../tools/rhino_oracle/fixtures/mesh_vertex_snaps.json)
and [owned Rhino observations](../tools/rhino_oracle/observations/mesh_vertex_snaps.json)
pair Vertex, Point and End at one mesh corner with the wire switch off and on.
Only Vertex captures, with the exact 3D vertex and a `MeshVertex` component.
Native replay agrees on snap kind, source and point in all six cases. Vertex
also wins the [three mixed-mode picks](../tools/rhino_oracle/fixtures/mesh_vertex_mixed_snaps.json)
against Near, Mid and Point; the [Rhino results](../tools/rhino_oracle/observations/mesh_vertex_mixed_snaps.json)
match native replay. [Three aperture picks](../tools/rhino_oracle/fixtures/mesh_vertex_aperture_snaps.json)
with [Rhino results](../tools/rhino_oracle/observations/mesh_vertex_aperture_snaps.json)
keep Vertex inside the capture box even when Near is closer to a wire, then
switch to Near once Vertex leaves the box. Back-face culling, competing mesh
objects and SubD vertices are still outside this measured scope.

## Evidence and known differences

The [16 inputs](../tools/rhino_oracle/fixtures/mesh_snaps.json) and
[complete observations](../tools/rhino_oracle/observations/mesh_snaps.json) retain
real perspective SplitEdge picks in Rhino 8.32.26160.13001. Runs used an owned
private Xvfb and public commands/APIs, never an existing desktop or Rhino process.
Each prompt records its camera matrix, integer click and mesh-switch restoration.
Twelve cases performed actual Undo/Redo; four controls did not split.

Independent [Fraction-based references](../tools/rhino_oracle/references/mesh_snaps.py)
compute wire targets from source geometry and recorded camera/click only, never
from Rhino result coordinates. All seven mesh captures now drive complete ordered
geometry, attributes, selection and history comparisons at absolute `1e-9`,
relative `1e-10`:

- Two captures are Mid and five are Near. The five previous perspective Near
  discrepancies (`2.325e-5` through `1.1945e-3`) came from incorrectly using curve
  Near's screen-Euclidean metric for mesh wires. The independently calibrated
  mesh-specific depth weighting resolves all five without changing the epsilon.
  The historical screen-nearest reference and raw geometry remain unchanged;
  corrected reference targets are retained separately.
- Nine controls establish mesh non-admission only. They do not assert that the
  entire scene has no snap: the receiving box can supply a curve Mid. Native tests
  check both the full-scene winner and an isolated mesh query. These controls do
  not replay the unsnapped or other-source positions as parity matches.

SplitEdge constrains targets to an x-axis edge. The Rhino observations therefore
establish only the target's constrained x coordinate, not its unconstrained 3D
location. The subsequent [unconstrained calibration](point-snaps.md) records
actual 3D targets, snap kinds and source ownership in 101 further cases. Ninety
captures match in full 3D; eight verify non-admission. Three corner-edge priority
differences remain explicitly tested, not treated as geometric parity passes.
The [wire-competition follow-up](mesh-snap-order.md) adds 48 competition cases
and 36 repetitions, exposing broader vertex-order and parallel-wire selection
differences. Its native replay reports mismatches as failures.

For mesh Near, an endpoint inside the square snap aperture selects ordinary
screen-distance interpolation. Otherwise the measured wire calculation uses
opposite-end depth weighting; parallel queries reduce to the affine solution.
The [derivation and independent corpus](point-snaps.md#measured-per-wire-calculation)
keep this separate from curve Near, whose Euclidean calculation is unchanged.

Two additional [switch fixtures](../tools/rhino_oracle/fixtures/mesh_snap_switches.json)
retain [14 states](../tools/rhino_oracle/observations/mesh_snap_switches.json),
including 12 Enable/Disable/Toggle transitions. All states match exactly and
preserve other settings. The probe reads and verifies the switch through its
English public command prompt; it fails explicitly on an unrecognized prompt.
This oracle helper is locale-specific, not a production Rhino API binding.

## Cache and performance boundaries

`ObjectSnapOptions` carries feature modes and mesh-source policy explicitly.
`object_snap/mesh` lazily builds a median-split bounds hierarchy over canonical
wires, with at most eight wires per leaf. The hierarchy is camera-independent;
projected bounds reject distant subtrees. Unprojectable bound corners disable
culling instead of risking a false rejection. Exact-distance ties retain original
canonical edge order despite hierarchy partitioning.

Shared geometry snapshots make unchanged-source checks constant-time. Edits and
Undo rebuild changed entries; deletion or conversion evicts them. Tolerance and
camera changes do not rebuild the mesh index. Suspended feature queries return
before cache traversal, and modes with neither Mid nor Near do not build an index.

A 4,096-quad regression contains 16,384 wires: each of 100 separated warm captures
visits fewer than 64 wires and the index builds once. A separate differential test
compares hierarchy queries with exhaustive individual wire queries under perspective
clipping, including midpoint targets and ties. A separate 630-case exact-rational
corpus checks mesh Near interpolation. These are traversal/correctness
checks, not timing measurements or a Rhino performance comparison. Cold topology
construction, overlapping projected bounds and scene-object traversal remain
unbounded by a fixed frame budget. Occlusion, general cross-object priorities and
full Rhino mesh wire-selection parity remain incomplete. Live point probes
cover fully visible wires, not arbitrary clipping or extreme coordinate ranges.

All four viewport types exercise ordinary and edge-constrained mesh targets;
real menu events preserve partial command input. Parser/state tests cover grammar,
idempotent switches, one-shot lifetime and model-history isolation.

## Reproduction

Initial checkpoint (`25ba7ba`): 3,137 ordinary release-workspace tests passed (29 opt-in
tests ignored), all 306 Python tests, and all seven offscreen GPU tests. Formatting,
whitespace, strict Clippy and strict rustdoc checks also passed.

```sh
cargo test --release -p viboceros-drafting object_snap::mesh
cargo test --release -p viboceros-oracle calibrated_mesh_snaps
cargo test --release -p viboceros mesh
python3 -m unittest tools.rhino_oracle.test_mesh_snaps
python3 -m tools.rhino_oracle.references.mesh_snaps
```

For live calibration, generate requests with `--artifact /owned/box.3dm` using
the verified box recipe/hash in provenance, then use the owned
`tools/rhino_oracle/run_headless.sh rhino REQUEST --timeout 600` runner. Generate
mathematical targets separately with `--observations RESPONSE --mesh-weighted`; only recorded
camera/click data are read. Raw native replay rejects unresolved mesh-pick aims.
The calibrated test first computes native targets and then passes their model
coordinates to the command adapter as resolved point inputs.
