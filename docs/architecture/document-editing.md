# Document objects and geometry editing

[Architecture overview](../architecture.md) · [Documentation](../README.md)

Implementation notes for this subsystem. Command pages and qualification records
describe the supported behavior and measured limits.

The document's [native block catalog](../block-definitions.md) retains shared geometry
definitions and nested affine references, with atomic graph validation, history,
bounded placement resolution and unit rescaling. [Instance objects](../block-instances.md)
retain references through affine editing, display and selection, with atomic
catalog/cache replay. [Block and Insert](../commands/blocks.md) now connect source
selection, base/name getters and scaled/rotated placement to that model.
[Structural 3DM interchange](../three-dm-structural-blocks.md) retains supported embedded
definitions and references. Linked/external files and broader metadata remain
subsequent work.
[Block expansion](../commands/explode-blocks.md) supplies one-level Explode and
recursive ExplodeBlock, with freshness checks, native group policies and one-step history.
[BlockManager](../commands/block-manager.md) adds definition inspection, linear graph
use counts, direct root selection, rename and transactional definition deletion.
[CreateUniqueBlock](../commands/create-unique-block.md) duplicates embedded definitions
and rebinds roots transactionally. [ReplaceBlock](../commands/replace-block.md) changes
definitions through typed names, target-instance picking or a searchable chooser,
retaining root placement, metadata and history with native replacement policies.
[AddObjectsToBlock](../commands/add-objects-to-block.md) appends model objects in a
selected instance's local frame, refreshes all uses, retains nested references
and consumes sources in one transaction.
[BlockResetScale](../commands/block-reset-scale.md) adjusts placements in the
instance frame, retains reflected/sheared behavior and preserves shared catalogs.
[BlockEdit](../commands/block-edit.md) exposes direct prototype members in an isolated
world-space workspace with copying, member release and base-point controls.
Save refreshes every root in one history entry; discard restores the original
document and history. External-source viewport picking keeps a separate highlight
set, base-point input uses the shared drafting pipeline, and idle block double
clicks open the editor. Nested contexts use composed frames, atomic scene
replacement and accepted context checkpoints; broader input/history qualification
remains open.
Restricted block members have explicit release or show/unlock save choices.
An empty accepted workspace retains its prior definition while accepting model
releases; native close-time warning choices are captured separately.
[Block object snapping](../block-snapping.md) reuses the existing feature/intersection
queries through immutable member records, preserving root ownership and bounded
insertion-point discovery.

The [Smooth kernel](../smooth.md) averages curve and surface controls and mesh
vertices in World, CPlane, and Object axes, with 304 native geometry replays.
The registered command supports atomic object/grip edits, remembered options,
selection prompts, and Undo/Redo; application tests replay those 304 cases.
The mesh containment query validates closed manifold topology once, classifies
surface boundary points, and uses retried ray parity for interior tests. Volume
object selection uses that query for mesh and tessellated B-rep sources.
The [TweenSurfaces workflow](../commands/tween-surfaces.md) adds control-net
transitions and normalized-UV sampled tensor interpolation, with explicit
direction changes and measured layer/attribute/group policy. Sampling accepts
unequal polynomial/rational nets and retains mean-chord output domains. Rational
control displacement retains the first weights with square-root weight ratios.
Ordered single-source picks and a readonly cached scene keep preview geometry
outside document history. Typed option/direction edits rebuild only when values
change; acceptance reuses prepared geometry in one transaction. Native source
preselection survives acceptance/cancellation and independent history.
End-surface corner controls provide measured U/V swap and U/V reversals.
Repeated native sequences retain reversal flags in their slots on swap. Queued
corner events revalidate sources/settings before changing the prepared state.
Their projected hit tests are independent of snaps, and transformed positions
are cached with the readonly preview rather than recomputed per frame.
Command-instance option storage applies measured field lifetimes: accepted
count/method/sample edits and immediate options-phase layer edits. Source/direction
state stays per invocation; history never changes saved preferences.
Refit matches degrees and knots for unequal positive rational nets and retains
the end source domains. Unequal control nets use uniform tensor interpolation
at midpoint-isocurve arc-length stations, rebuilding before every output as
measured in 38 native cases. Compatible nets keep first-source knots. Bounded
faer solves check pivots, finite controls and residuals. Native option-change
object effects, general periodic/singular sources and performance parity remain
unresolved. Complete command/SDK evidence and a sampled Python geometry query
retain those boundaries.
The shared [surface Rebuild kernel](../commands/rebuild-surfaces.md) also backs the
surface branch of `Rebuild` and the Python `surface_rebuild_geometry` operation.
Twelve native commands establish replacement identity, copied attributes,
cleared output groups, layer choice and independent history. A separate twelve
SDK queries check asymmetric target structures and reductions. Numeric app
prompts share command-owned parsing and immediate remembered options; two native
sequences establish cancellation/history lifetime and ordered degree/count
coupling. Physical retrimming projects original edges onto the changed surface,
preserves source domains and certifies newly constructed trim/edge correspondence.
Fourteen native commands distinguish projection from UV copying, including
nonuniform, curved/rational and holed sources. Projection fitting has sampled
accuracy checks; interior singular trims, native preview appearance, restart persistence
and performance parity remain unfinished or unverified. Rebuild options use a
readonly scene and a shared prepared command result. Source snapshots and metadata
guard acceptance; unchanged edits retain the cache, output policies reuse geometry,
and background edits refresh the scene. Natural four-side faces construct exact
target isocurves directly, avoiding inverse-fit work. A failed later preparation
clears the preview as one unit and blocks acceptance until recovery.
Closed Rebuild directions use cyclic uniform interpolation with exact repeated
controls. Constant boundary isocurves stay exact poles, and natural seam/singular
faces retain shared incidence on the rebuilt surface. Twenty-eight native command
and SDK records cover primitives, swapped charts and degree parity. Trimmed closed
faces keep per-use periodic branches and shared seam incidence; ten native cylinder
recipes extend that evidence to patches and holes crossing the seam. Compact fits
retain original-span validation stations, while unshared contours can subdivide
into continuously certified UV/spatial pieces. Extra boundary segmentation remains
an explicit native representation difference. Exact collapsed natural sides also
retain full or partial singular UV trims and pole vertices without spatial edges.
Twelve native cap/hole/wedge recipes cover spheres, a cone and edited charts.
Perturbed target poles are rejected before document edits. ReTrim=No normalizes
newly closed single-face solids outward, using exact sense where available and a
numerical signed-volume fallback for an unresolved connected shell.
A full constant-U contour on a closed V chart transfers to the target isocurve
at its projected endpoint U. Independent closest-point queries expose the native
swapped-cap asymmetry relative to pointwise projection.
Interior singular trim projection,
multi-chart winding and arbitrary periodic surfaces remain unfinished or unverified.
Curved cubic trim/image certificates use outward interval Bernstein products and
restricted knot-crossing boxes, retaining exact rational extraction and fallback.
The [retrim profile](../retrim-performance.md) records a default sphere-cap reduction
from 64.78 to 1.61 seconds with identical recorded geometry; native SDK timing
still shows a substantial gap. Overflow, subnormal and uncertain proofs use the
preceding exact path, and hidden between-station excursions remain rejected.
The Pipe command reuses exact axial NURBS primitives for straight rails and
the rotation-minimizing Sweep1 kernel for smooth curved rails. Planar B-rep
hole capping closes the swept ends when requested. Thick pipes combine outer
and reversed inner walls before planar annulus capping.
Its [compact exact-product fallback](../compact-exact-products.md) selects bounded
integer windows without weakening rounding or full-range recovery.

| Module | Responsibility |
| --- | --- |
| `viboceros-geometry` | Validated primitives, NURBS, intersections, B-rep topology, tessellation, and mass properties; nalgebra and faer provide linear algebra. |
| `viboceros-document` | Objects, attributes, layers, groups, selection, transactions, and bounded undo/redo. |
| `viboceros-drafting` | Snapping and tracking calculations. |
| `viboceros-io` | STL, OpenNURBS 3DM bindings, and initial STEP interchange. |
| `viboceros-command` | Document commands plus a separate, document-independent interface-command parser and state reducer. |
| `src/` | egui application and wgpu viewport rendering. |
| `viboceros-oracle`, `tools/rhino_oracle/` | Matching native and public Rhino API probes and a Python comparison client. |
| `third_party/` | Pinned OpenNURBS source and its license. |

The [Align command](../commands/align.md) separates option parsing, rigid layout,
and interactive mode/point/target-curve phases. Its `layout_units` helper shares top-group
selection with Distribute; both use `object_bounds` for geometry-local bounds.
Projection alignment has a separate per-object bottom-center path: group units
do not apply. Prepared `PointProjection3` definitions preserve exact endpoint
differences and plane normals, sharing finite rational conversion with NURBS
evaluators through the kernel's independent `exact_scalar` module.
Best-fit projection adds exact affine-rank checks and centroid accumulation,
shared-denominator matrix normalization, and faer thin SVD; see [plane fitting](../plane-fit.md).
Curve targets are immutable document-ID references, independent of moving-source
selection. The shared viewport filter includes curves-only single-target picking.
Ellipse closest points use a monotone quadrant equation with exact coefficient
formation; composite candidates share the kernel's exact distance ordering.
Failed command-first selection cleanup is a separate registry hook that runs
after model rollback, rather than a model edit inside a failed transaction.
The oracle's `object_layout` module shares owned fixture setup and output
recording without sharing the native and Rhino alignment implementations.
