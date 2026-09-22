# VolumeCentroid

[Command index](README.md) · [AreaCentroid](area-centroid.md) · [Mass integration](../mass-properties.md)

Select surfaces, B-reps or meshes and enter `VolumeCentroid`, or enter the command first,
pick objects, and press Enter. One point marks the cumulative **signed-volume**
centroid, not the surface-area centroid or a vertex average. Groups do not
partition the result or multiply contributions; directly selected group subsets
contribute only their selected members.

Closed objects execute immediately. If any selected boundary is not closed,
the UI asks whether to continue: volume is meaningful only when the selected
objects jointly enclose it. Yes, Enter and Escape at this warning continue; No
declines without a marker or model history change. Escape while picking objects,
or replacing the command, cancels without calculation. The answer is not remembered.
Headless callers must explicitly use `VolumeCentroid Continue=Yes` for open input;
bare execution reports that confirmation is required. A supplied answer has no
effect when closed input needs no warning.

The command skips curves and points in mixed preselection. Consistent orientation
is required for an actual enclosed volume, but the command's warning follows
topological mesh closure rather than certifying its winding, as Rhino does.
Inconsistent winding and isolated open pieces produce signed reference-dependent
flux, not a validated solid. The [42-case confirmation audit](../volume-centroid-open.md)
retains the original timeout, actual warning behavior, and the isolated bilinear
surface counterexample. General open-surface compatibility is not claimed.
Native warning/command replay matches 38/42 cases at `1e-9`; the four retained
failures are the isolated bilinear surface's marker coordinates.

Reversing a complete shell changes its signed volume and first moments, but not
its individual centroid. Oppositely wound objects subtract when combined, so the
cumulative centroid can lie outside their bounds. Exact signed-volume
cancellation succeeds without creating a marker or model undo step.

Successful nonzero queries add one unselected, unnamed, ungrouped point on the
current layer and create one undo step. Sources and attributes stay unchanged.
Preselection is retained, including excluded objects; command-first completion
clears selection, including on zero-volume success and a declined warning. No eligible geometry fails
and clears selection. Numerical failures leave no partial marker; Esc cancels
picking. Native tests exercise actual UI picking and repeated undo/redo.

## Kernel

`VolumeMassProperties` owns signed volume and three signed first moments,
independently of document state. Mesh tetrahedral contributions are exact for
the stored binary64 coordinates, using cubic and quartic integer accumulators.
The hot accumulation loop allocates no arbitrary-precision objects; final rational
conversion and topology validation do allocate. Translation and cancellation
never discard small volume contributions, and a finite centroid can survive an
underflowing or overflowing binary64 volume. Final coordinates round once.

`VolumeBoundary` and `VolumeMassProperties::from_boundaries` integrate separately
represented boundary pieces using the center of their combined bounds. Unused
mesh vertices do not influence that base. `volume_flux(base, ...)` exposes an
explicit-reference kernel query; the original `volume_mass_properties` methods
still require oriented solids. Reference-dependent cone determinants are expanded
algebraically into exact cubic/quartic monomials, never computed by rounded
vertex-minus-base subtraction. This also handles differences outside binary64
range. Closed oriented objects retain their reference-independent fast paths.

B-reps integrate divergence-theorem densities on exact rational surfaces and UV
trims. All faces share one centered/scaled spatial frame, including cavity shells;
recentring faces independently would invalidate their flux cancellation. Area
and volume moments share bounded rectangle quadrature and spatial conditioning;
trimmed moments reuse the existing Green-theorem boundary traversal. These are
numerical integrals, not certified exact curved volumes. Nonconvergence, work-limit
exhaustion, and lost coordinate range return errors. Arbitrarily ill-conditioned
UV domains are not guaranteed to succeed.

Mesh `Volume` and `VolumeCentroid` now use the same shorter spatial quad diagonal
as `Area`; exact ties retain A-C. This corrects the old fixed-display-diagonal
volume calculation on warped quads. Display triangles and stored face order
remain unchanged. Eight closed warped-box winding/rotation controls preserve
volume magnitude 26 and centroid `(27/13,81/52,57/52)`.

Exact ambiguous-diagonal comparison now uses fixed-size integer sums, shared
with point-distance ordering, instead of per-face rational allocation. In the
[earlier local release benchmark](../volume-centroid-performance.json), first moments
for 6,144 quads fell from 18.2 to 5.25 ms at the origin and from 41.6 to 13.1 ms
at translation `1e12`. These are median-of-seven before/after implementation
measurements, not Rhino timings or a portable performance guarantee.
After shared-reference support, the same closed-mesh check recorded 5.95 ms and
14.0 ms respectively; all rows are retained. Open-reference throughput and Rhino
performance parity are not established by this check.

## Evidence and known differences

The [26 source-only fixtures](../../tools/rhino_oracle/fixtures/volume_centroid.json)
and [complete public Rhino observations](../../tools/rhino_oracle/observations/volume_centroid.json)
cover tetrahedra, translation, grouping/subsets, pre/postselection, mixed winding,
zero cancellation, unsupported input, three trimmed solids, and warped quads.
Captures use Rhino 8.32.26160.13001 in an owned private-Xvfb session.
All source definitions and mesh binary64 coordinates are retained and checked
unchanged by the command. See [provenance](../volume-centroid-provenance.json).

At absolute `1e-9`, relative zero, command replay passes **26/26** cases (maximum
coordinate difference `3.56e-15`); constructed-source API replay passes **14/26**.
These have deliberately separate scopes. Rhino's
`VolumeMassProperties.Centroid` returns the origin for the nine negative-mesh
fixture occurrences, despite valid signed first moments and a correctly placed
actual-command marker. For example, the reversed tetrahedron has volume `-10`,
first moments `(-7.5,-10,-12.5)`, and command centroid `(0.75,1,1.25)`.
Native results retain the mathematical centroid rather than copying the getter
anomaly. Three curved-solid default API volumes also exceed the `1e-9` comparison
threshold; native tests use independent radial integrals.

Rhino reverses the inward B-rep on document insertion. The harness verifies a
complete public duplicate-and-Flip match and retains both geometry definitions,
their separate API results, and the actual command result. Source API replay
uses the constructed geometry, never silently substitutes the post-insertion
orientation, and never passes observed targets to the native engine. Source
attributes, exact history strings, and Rhino Undo/Redo are not replay targets.

```sh
# Actual command result/state/marker attributes.
python3 -m tools.rhino_oracle.volume_centroid_replay \
  tools/rhino_oracle/fixtures/volume_centroid.json \
  tools/rhino_oracle/observations/volume_centroid.json
# Add --source-api to expose retained raw-API differences (exit 1).
python3 -m unittest tools.rhino_oracle.test_volume_centroid
# Optional local throughput check; not a Rhino performance-parity claim.
cargo run --release -p viboceros-geometry --example volume_mass_bench
```

Independent tests cover full-range exact monomials, tiny/huge mesh volumes,
large translations, off-center cavities, closed rational spheres, cylinders,
trimmed paraboloid solids, winding, and unchanged geometry. The shared oracle
helper was also rerun on all 54 existing AreaCentroid fixtures: recorded geometry
and compared outcomes were unchanged.

See the official [VolumeCentroid command](https://docs.mcneel.com/rhino/8/help/en-us/commands/volumecentroid.htm)
and [public mass-properties API](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/T_Rhino_Geometry_VolumeMassProperties.htm).
