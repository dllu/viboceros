# Command construction and staged input

[Architecture overview](../architecture.md) · [Documentation](../README.md)

Implementation notes for this subsystem. Command pages and qualification records
describe the supported behavior and measured limits.

The command crate's `execution` module owns invocation stages, dispatch and
transaction boundaries. It distinguishes commands whose object getter already
started, so finishing selection does not repeat command-start history effects.
The root defines the public command contract and registry; `error` owns shared
`CommandError` variants. ExtractPt separates option parsing, component resolution and
atomic output admission; its application controller supplies witnessed component
picks to the same typed command path.

[`RemapCPlane`](../commands/remap-cplane.md) shares rigid frame mapping and the
existing object/grip transform path. Its application controller captures the
source frame and resolves viewport, named CPlane and saved-view targets without
editing cameras or planes. Native command captures constrain Bottom orientation,
Copy cancellation, source/group policy and history.

[IPlane](../commands/iplane.md) reuses the Section intersection producer and
Project plane picker for standalone unbounded cuts, with staged output and one
history edit. Its Python oracle routes geometry to Rhino 8 public plane APIs;
fresh native command/getter parity remains unqualified.

[Section](../commands/section.md) adds CPlane-aware plane cuts with staged output,
property/group options and repeated two-point input. Surface/B-rep intersections
reuse the kernel; mesh sections use exact dyadic edge intersections and shared
topology tracing. Coplanar mesh parameterization retains a native diagnostic.

[Contour](../commands/contour.md) adds a 3D normal, equally spaced planes, picked
ranges, per-plane groups and unit-aware viewport input. Exact rational grid
indices and origins avoid repeated spacing addition and support remote base
points. Surface domain selection preserves edge/isocurve lineage and signed plane-line
parameters; all 52 recorded Contour captures now match strict replay. Broader
curved/B-rep, seam and performance qualification remains open.

Typed coordinates are resolved by `viboceros-drafting/point_input`, independent
of UI and document edits. `app/point_input` shares the interactive command's
point-validation and transaction path with mouse picks; see [point input](../point-input.md).
`viboceros-drafting/point_filter` composes filtered CPlane/world coordinates
from a source pick and a final pick. The viewport previews that composite while
passing the original pick to the app for one-time resolution.
`viboceros-drafting/point_constraint` applies one-pick 3D distance locks and
CPlane angle tracking after the filter resolves. Owned Rhino mouse probes pair
free and constrained clicks at the same screen pixels for numerical replay.
`viboceros-drafting/object_snap/intersection` resolves visible straight
crossings in camera space and back-projects each candidate onto its 3D source.
Mesh candidates share the snapshot-cached wire hierarchy.
The command crate's `interchange` module owns the six STL/STEP/3DM import/export
commands, document-to-file attribute/group adapters, and export mesh aggregation.
Parsing and file replacement remain in `viboceros-io`; registry transactions
remain responsible for rolling back failed imports. Existing mesh aggregation
uses numerical validation, independently of surface tessellation tolerance.
`CommandContext` carries an explicit construction plane separately from document
history. The [plane-primitives module](../construction-planes.md) handles orientation,
projection, and native parameter policy for six primitive commands.
The separate [plane-transforms module](../plane-transforms.md) handles reference
interpretation and affine maps for four transforms. The
[Mirror module](../commands/mirror.md) owns two-point, three-point, CPlane-axis, and
planar-object interpretation; its app adapter owns options, target picking, and
point phases. The transform session keeps the original source IDs when Object
selection clears highlighting, passing them through an internal `Sources=`
registry argument independently of the plane target.
Both share document mutation and copy/group transactions with other affine commands.
Move/Copy, Mirror, and Scale/Rotate/Shear use a shared affine display-instance
path in `viewport/scene`. The command crate's `point_transform` module resolves
point-prompt maps without editing the document or command defaults. Prompt
sessions keep the displayed map; the viewport resolves cursor constraints and
reuses cached source samples, meshes, and normals. A prepared kernel cofactor
map supplies normals under nonuniform scaling, shear, and reflection. Previews
never install geometry snapshots or history entries; see
[display caching](../viewport-caching.md), [Move/Copy previews](../commands/move-copy.md#live-previews),
and [affine previews](../commands/affine-previews.md).
The [Twist command](../commands/twist.md) shares the general point-morph fitters
with preserved-control overrides and measured rigid placements. Its prompt
adapter retains source IDs and groups repeated copies into one history entry.
Command-instance preferences remember successful options outside document history.
The command adapter applies the measured `1e-5` fitting floor independently of
the kernel's explicit fit contract. A document replay marker retains explicit
source reselection between Undo and Redo without changing ordinary source-pick
cleanup or unrelated selection.
Its viewport adapter uses prepared control cages, quick interpolation grids and
cached trim intervals for wire previews. Rigid instances share source display geometry.
[ScalePositions](../commands/scale-positions.md) uses a shared rigid placement module
with individual tight centers, while ScaleNU keeps its grouped layout. Atomic
whole-object replacements retain picked grip indices; its copies preserve geometry
metadata and join the source groups. Application replay covers modes, numeric getter
phases, defaults, source order and history against retained public native outputs.
The [Bend command](../commands/bend.md) uses the circular spine morph and the same
fitters, with atomic staging and grouped rigid poses. Its application adapter
retains source IDs for repeated copies and distinguishes immediately remembered
spine/angle choices from options saved on successful completion. A scoped history
finalization policy retains preselection for canceled copies through Undo/Redo.
The [geometry oracle](../bend-geometry.md) retains point and command witnesses;
[live previews](../bend-preview.md) share prepared display cages with Twist,
including cached boundary variants for Shaded and Ghosted views.
The [Taper kernel](../taper-geometry.md) adds radial and flat point maps with cubic
finite-axis blending, linear extrapolation, extreme-range fallbacks and measured
rigid poses. Its [command adapter](../commands/taper.md) resolves signed distances
and construction-plane or picked Flat directions, stages geometry atomically,
and retains sources for repeated copies. All flags save on successful outer
completion, outside document history. Its [live preview](../taper-preview.md) shares
prepared morph cages, preserves pending isocurves in all display modes and uses
an axis-normal radius mouse plane with an edge-on construction-plane fallback.
The [Circle fit foundation](../circle-fit.md) adds spatial-distance fitting on a
PCA plane with retained native command/SDK evidence. Native basis conventions
and near-collinear convergence remain unresolved. Its
[point and grip command input](../circle-fit-input.md) uses the shared filtered
selection prompt and retains native cancellation/Undo/Redo behavior. Shared
[control-point state](../control-points.md) retains source identities and immutable
geometry snapshots for viewport picking and Circle grip input.
[Curve edit-point mode](../commands/edit-points.md) shares that state and preview
cache, reconstructing transformed Greville stations in the original NURBS basis
with the existing interpolation and exact rational station checks.
[Partial grip transforms](../grip-transforms.md) stage mixed whole-object/control
edits through a shared geometry operation; history exchanges grip display and
picks. The viewport shares immutable partial preview geometry across views.
The [Maelstrom foundation](../maelstrom-geometry.md) adds cylindrical spiral maps,
including the measured relative-radius threshold and raw tiny-angle rotations.
It shares fitting and rigid placement with the other morph kernels. Its
[command adapter](../commands/maelstrom.md) stages selected geometry, resolves
Circle plane/radius picks and projected second radii, supports Copy/Rigid and records unchanged replacements.
Circle radius and Copy getter edits have their own preference policy; staged
interactive input supports source selection, radius and coil-angle getters, and
repeated Copy placements in one Undo batch. Its [live preview](../maelstrom-preview.md)
shares prepared morph cages, retains circle guides and full mouse turns, and
keeps pending surface isocurves in all display modes. Its independent
[first-circle getter](../maelstrom-circle-input.md) resolves construction and size
modes without model edits, carries accepted frames into drafting and morphing,
and keeps Radius/Diameter preferences outside document history.
The document's independent `groups` module owns ordered object memberships and
their reverse member index. Lightweight membership history records preserve
order without cloning geometry; see [group invariants](../groups.md).
The `object_properties` module keeps attribute and isolation history separate
from geometry; see [property history](../history-selection.md#property-history).
The document's `geometry_snapshot` module shares immutable geometry between live
objects, history, clones, and [display caches](../viewport-caching.md). Edits replace
handles, so cache invalidation uses retained storage identity without rescanning
geometry. Value equality remains structural; no mutable geometry handle is exposed.
The [object-snap caches](../snap-caching.md) reuse those snapshots, inspect feature-specific
source changes only after replacement, and index object/layer lookups per query.
The independent `object_snap/mesh` module caches a camera-free bounds hierarchy
over exact-location face-boundary wires. `ObjectSnapOptions` supplies feature
modes and mesh eligibility without coupling cache contents to UI lifetime;
[mesh snapping](../mesh-snaps.md) documents calibration and precision limits.
The [batch-deletion module](../batch-deletion.md) moves removed objects into compact
history records and restores table order without repeated vector insertions.
The [ordered selection iterator](../selection-iteration.md) shares efficient,
read-only selected-object lookup across commands without cloning geometry.
The [bulk object resolver](../bulk-object-lookup.md) shares temporary table indices
and missing-ID validation across transforms, copies, and editable attributes.
The `object_copy` module owns affine arrays, morph copies, and replacement
geometry copies, including their group policies and shared instance commit.
Repeated-source piece copies preserve every input occurrence and share the
replacement-copy commit, resolving and validating distinct sources once.
Layer transfers remain in `object_layer`; both use indexed group assignment.
The independent [arrays module](../plane-arrays.md) owns rectangular, linear, and
polar layout. Its geometry extents use tolerance-controlled
[curve](../curve-bounds.md), [surface](../surface-bounds.md), and
[trimmed-face bounds](../trimmed-face-bounds.md), separately from
document/control-net bounds. The geometry `bezier` module owns local homogeneous
span extraction shared by [Bézier conversion](../commands/beziers.md), directional
[single-span conversion](../commands/single-spans.md), and bounds. Each conversion
command has its own module and typed [option memory](../command-options.md), outside
document history. Shared oracle recording lives in `conversion`; self-seeded
stateful probes live in `conversion_session`, separate from geometry algorithms.
The separate [ToNURBS module](../commands/to-nurbs.md) reuses exact curve representation
and mesh-to-B-rep construction. Its replacement order is handled by the document's
`object_order` module, with linear-time permutation replay and lightweight history
containing only moved IDs and original indices. Geometry, memberships, and
selection action order are independent of chronological object order.
[`MeshToNURB`](../commands/mesh-to-nurb.md) has a separate command module and option
memory. Mesh connectivity, polygon normals, edge collapse, and edge splitting
have separate kernel modules. They preserve raw-vertex seam distinctions while
sharing exact-location topology. See [mesh topology and editing](../mesh-topology.md)
for allocation policy, numerical boundaries, and independent/Rhino regression coverage.
The command crate's `mesh_edit` modules own face edits, edge edits, hole filling,
triangulation and computed extraction. Their existing regression cases are grouped
by operation under `tests/mesh_edit`; the registry retains command registration.
The kernel's `mesh/ngon` module validates logical face groups over triangle/quad
storage; the 3DM bridge's `mesh_ngon` payload preserves their boundary and member
indices. Viewport display uses visible n-gon edges while editing selectors keep
the complete topology edge order.
The `split_disjoint_mesh` command module owns staged component results, fresh
piece insertion, source deletion policy, attribute/group propagation, and selection.
The separate `explode` command module owns multi-geometry decomposition staging,
output summaries, source removal, and result selection.
Its `summary` module accumulates bounded category counts during staging and
formats the fixed-order report, without rescanning generated parts per category.
Its document-independent `parts` module owns command-specific component order,
polycurve flattening and parameter intervals, and conversion to output geometry.
Selection, source retention, attributes, groups, and history stay in the command.
The command-owned `object_selection` module supplies typed filters and boolean
options to the separate [app object-prompt adapter](../object-selection.md).
`selection_commands` owns basic, action-history, attribute, and duplicate
selectors together with their argument parsing and remembered selection options.
The document's `wildcard` module shares name matching between object and layer
selection. Its greedy matcher is checked against an independent prefix-table
reference for 116,281 short pattern/name pairs, including names containing
literal wildcard characters. ASCII candidate names are matched directly from
borrowed bytes with on-demand lowercase conversion and no per-candidate heap
allocation. Non-ASCII names retain whole-string Unicode lowercasing (including
contextual and expanding mappings) before matching. Both paths share the same
backtracking implementation, with additional reference tests covering all ASCII
bytes and mixed Unicode inputs. Worst-case backtracking remains O(pattern length
× name length); these tests are not a claim of general Rhino Unicode parity or
an end-to-end performance measurement.
`object_name` owns `SetObjectName`, keeping quoted literal names distinct from
options and preserving quoted internal whitespace before document assignment.
The independent `geometry_selection` command module owns document-wide type,
topology, planarity, and short-curve selectors. It preflights eligible matches
before applying additive selection and leaves model history unchanged.
`Intersect` and the separate `intersect_two_sets` command share one curve,
surface, and B-rep pair evaluator. The two-set command admits only cross-set
pairs and stages all outputs before editing the document.
The [native self-contact adapter](../smlib-curve-self-intersections.md) replaces
IntersectSelf and SelSelfIntersectingCrv's recursive search for eligible curves,
sharing paired-parameter acceptance and segment extraction while preserving the
document policies. Nested collinear Rhino parity remains under qualification.
The [native curve/surface producer](../smlib-curve-surface-intersections.md) shares
original-geometry acceptance and temporary surface import with curve/B-rep
queries. Face trim containment and projected UV charts stay in one geometry
policy, while command selection, source replacement and history stay separate.
The app's two-set prompt snapshots the first picked set before clearing the
transient selection for the second; both phases use the parametric-object filter.
Boolean pickers reuse its routing while owning their filters, hints, options,
and cancellation policy in `app/boolean_solids`. Its `input`, `options`, and
`selection` modules preserve ordered picks and command-specific history behavior.
The geometry [shortness module](../curve-shortness.md) keeps representation-dependent
selection integration separate from accuracy-controlled arc-length measurement.
The [`measurements` command module](../commands/measurements.md) owns read-only
Length, Area, and Volume, with shared streaming exact finite-value aggregation and
separate nonnegative/signed policies. Its tests live beside the implementation.
Its separate [`area_centroid` command](../commands/area-centroid.md) owns filtered
selection and atomic marker creation. The kernel's `area_mass_properties` module
owns exact weighted aggregation, mesh accumulation, and normalized surface
densities; B-rep moments reuse the existing trimmed-boundary traversal.
The independent [`volume_mass_properties` module](../commands/volume-centroid.md)
retains oriented volume moments. Shared `mass_integration` owns spatial frames
and rectangle density quadrature, with one common frame across a solid's faces.
`binary_accumulator::monomials` supplies allocation-free cubic/quartic sums for
mesh volume moments. Command marker/selection policy stays outside the kernel;
the oracle's `centroid_command` module shares owned fixture setup, not algorithms.
Its `distance` submodule handles read-only two-point distance, world/CPlane deltas,
and angular reporting. The application supplies picked or typed points through
the shared drafting path; neither implementation mutates document history.
Its `evaluate_point` submodule reports world/CPlane coordinates from an explicit
command context; the application adapter keeps failed point queries open.
Its `domain` submodule reports native parameter intervals, including the underlying
surface of an explicitly indexed or picked B-rep face. The app's `domain` adapter
continues accepted polysurface selection with a component-location pick.
`evaluate_uv` owns surface-coordinate query options and optional projected-point
creation. It shares one parser with the UI, uses underlying-face closest points
(ignoring trims), and delegates inverse domain mapping to `NurbsSurface::normalized_parameters`.
Its `angle` submodule owns the four-point Angle query, using range-safe
`Point3::direction_to` and `Vector3::angle_to`. Its `objects` submodule measures
unoriented straight-curve/planar-surface angles. The interactive adapter validates
each direction endpoint before advancing; `src/app/angle.rs` routes the initial
`TwoObjects` option into the shared selection workflow.
`curvature/radius` implements local Radius/Diameter reports and reuses Curvature's
differential evaluation and marker construction rather than fitting circles or
duplicating marker topology. The application supplies one curve-location pick;
preselection restricts the search, otherwise the document's selectable-object
iterator provides candidates. `src/app/radius.rs` retains failed point prompts.
`curve/area` dispatches enclosed curve area to analytic formulas or the existing
exact planar-face boundary integrator; temporary topology stays outside the document.
