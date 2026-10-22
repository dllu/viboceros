# Viewport, drafting and selection boundaries

[Architecture overview](../architecture.md) · [Documentation](../README.md)

Implementation notes for this subsystem. Command pages and qualification records
describe the supported behavior and measured limits.

The event loop delegates to small modules without putting camera or drafting
state into document transactions:

| Module | Responsibility |
| --- | --- |
| `viewport/camera` | Projection, drafting rays, navigation, and GPU camera coordinates |
| `viewport/extents` | Staged, atomic visible/selected bounds fitting |
| `viewport/screen` | Shared clipping, containment, and screen-distance predicates |
| `viewport/selection` | Object filters, click dispatch, and window/crossing selection |
| `viewport/picking` | Face hit metrics and depth interpolation |
| `viewport/component_picking` | Typed component clicks, rectangles, depth clipping, and highlight overlays |
| `viewport/polygon_edges` | Mesh/SubD topology edges and SubD vertex capture, including visible control cages |
| `viewport/edge_picking`, `viewport/edge_point` | Boundary-component capture and constrained screen-space edge locations |
| `viewport/scene` | Object display, shading, depth staging, and GPU buffers |
| `viewport/display_cache` | Shared immutable display meshes, normals, and wires |
| `viewport/mesh_normals` | Raw mesh sharing and surface tessellation smoothing |
| `viewport/drafting` | Cursor resolution, construction-plane grid, and drafting overlays |

The `viboceros-drafting` crate separates feature snaps (`object_snap`), plane-local
drafting (`plane`), and typed coordinates (`point_input`), with API regression
tests outside the crate root. See [viewport implementation and precision](../viewport-implementation.md)
for coordinate conventions, numeric safeguards, and their specific test coverage.
`object_snap/features` shares cheap curve-leaf enumeration; `object_snap/cache`
retains expensive NURBS arc-length and natural surface-boundary midpoints.
The [circular NURBS follow-up](../circular-center-snaps.md) shares each curve source
between independently lazy Mid integration and circular/elliptical Center
recognition; see [ellipse recognition](../elliptic-center-snaps.md).
`object_snap/polygon_centers` caches closed-boundary corner averages and planar
face recognition independently of camera projection. See
[composite feature snaps](../composite-feature-snaps.md), [analytic Center](../center-hover-snaps.md)
and [polygon Center](../polygon-center-snaps.md) for invalidation, independent numerical
tests, retained Rhino evidence and remaining capture limits.
`object_snap/projected_line` shares visible-interval clipping, screen distance and
inverse perspective interpolation between Near and Mid/Center hover admission.
It avoids uniform sampling for a straight locus with a projectable endpoint;
see [straight-line snap correctness](../projected-line-snaps.md) for the exact
reference corpus, numerical regressions and remaining projection limits.

`app/component_selection` keeps transient B-rep, mesh and SubD component picks and
ambiguity outside document history, with shared geometry snapshots to invalidate
stale indices. `app/untrim_holes` consumes preselection with remembered options,
then uses the same read-only preparation for postselection rectangles and its
command history group for immediate edits. Component selection never selects a
parent object or restores itself through Undo.
`app/crease` collects mixed mesh edges and SubD edges/vertices before one atomic
edit. Cached SubD edge samples follow public OpenNURBS refinement lineage; their
source ordinals remain independent of coincident world positions.

`brep/untrim/{selection,boundary,ordering}` classifies exact native UV trims,
restores connected outer runs or joined interior holes, and remaps rectangular
component indices through measured allocation classes. `command/untrim_edge` holds shared
source snapshots and stages geometry/retained objects before an atomic commit.
The shared `app/untrim_holes` prompt also drives Untrim, clearing its ignored
preselection and using independent remembered AllSimilar/KeepTrimObjects options.
See [Untrim coverage and topology limits](../commands/untrim.md).

`brep/unjoin` splits edge uses and local vertex fans with index tables and
union/find, then validates exact connected components. `command/unjoin_edge`
stages complete multi-object edits and metadata inheritance. `app/unjoin_edge`
applies eligible preselection immediately and collects postselection until
Enter, checking source snapshots before one atomic batch history record.
Independent Python capture and Rust replay modules compare public API and
actual command results from identically exported inputs; see
[edge separation](../geometry/unjoin-edges.md).

`app/edge_commands` shares component ambiguity, hover highlighting and stale
candidate checks for [MergeEdge](../commands/merge-edge.md) and
[SplitEdge](../commands/split-edge.md). Command-owned prepared selections keep
source/tolerance snapshots separate from document history. Merge chooses a
certified neighboring chain; Split collects original-edge locations and performs
one atomic subdivision when finishing, including when finishing with Escape.

Viewport hit-testing applies the filter before hit priority; clicks/windows use
the same selection adapter. Prompt choices live outside model history, while
finishing uses the registry's normal transaction lifecycle with explicit
postselection dispatch. Typed workflows distinguish options during selection from
ToNURBS's separate confirmation and MeshOptions submenu. The command determines
selection-dependent menus; the app owns their phase and pre/postselection origin.
Confirmation disables object picking without enabling point drafting. Nested CPlane
input and display controls retain the current phase. Explicit ordered document
renewal and ordered copies support pick-dependent creation order; inverse renewal
uses adjacent cycle swaps with the same single scratch permutation as forward replay.

Application interface regressions live under `src/app/tests/interface/`, grouped
by viewport layout, navigation, selection, drafting, prompts, shortcuts and toolbar
behavior. The parent module shares frame and input helpers across those topics.
