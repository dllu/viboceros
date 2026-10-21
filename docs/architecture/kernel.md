# Native kernels and numerical geometry

[Architecture overview](../architecture.md) · [Documentation](../README.md)

Implementation notes for this subsystem. Command pages and qualification records
describe the supported behavior and measured limits.

[Surface geodesic shooting](../surface-geodesic-offset.md) shares NURBS derivative
evaluation and the existing Cash–Karp rule. It normalizes charts and geometry,
checks regular transitions and retains model-space integration diagnostics.
Complete smooth offset loci reuse adaptive morph collocation and rational
composition after certified source pullback. Natural-boundary clipping shares
surface extension, UV fitting, intersections and trimming, with independent
model-space checks. Closed planar gap reconnection reuses conic and NURBS
concatenation operations. OffsetCrvOnSrf still needs command integration,
trimmed boundaries, general gap/corner handling and five SDK discrepancies
resolved.

The `viboceros-opennurbs` crate owns the shared OpenNURBS build and C ABI.
Its mesh adapters have no dependency on the Rust geometry or file-format crates;
geometry uses them for planar n-gons and coplanar face merging. File readers and
writers remain in `viboceros-io`, which links the same native archive. This avoids
a geometry-to-I/O dependency and builds OpenNURBS once per Cargo configuration.

Native modeling uses the separate `viboceros-smlib` adapter around usd-brep's
SMLib kernel. Rust retains document geometry and metadata; the adapter transfers
validated NURBS topology, serializes kernel calls and uses copied operands.
With `native-smlib`, BooleanDifference and BooleanUnion enter this backend
directly for planar and curved solids. BooleanSplit also enters SMLib directly
when all operands are closed solids; finite-plane and open-sheet workflows retain
the Rust path. Common Intersection enters SMLib directly; two-set Intersection
does the same when all inputs are closed. Open/sheet modeling still retains Rust
adapters, and removing those remaining duplicate routes is ongoing. Command contact, shell participation and history policies stay
outside the kernel's geometric operations. See [the native integration](../smlib-import.md).
Closed [Boolean2Objects](../smlib-boolean-two.md) choices also enter usd-brep directly
and reuse the same preview and document acceptance flow.
The [surface-fillet producer](../native-surface-fillets.md) now delegates offset
intersections, rolling-ball tracing and rational circular sections to SMLib.
It shares normalized surface preparation and exports owned sheet topology.
[FilletSrf](../commands/fillet-srf.md) adds ordered surface-side picks, cached previews,
trim/split parent ownership and native stitching as one document transaction.
Curved extended-fillet parity and general trimmed parents remain in progress.
[OffsetSrf](../smlib-offset-surfaces.md) now shares native sheet/solid transfer and
offset/shell operations; the planar offset adapter is excluded from native builds.
[Join and JoinCopy](../smlib-joining.md) also use native sheet stitching and edge
splitting; their command selection and document mutation policies remain in Rust.
[ExtrudeCrv](../smlib-extrusion.md) uses native surface and capped-sweep producers,
sharing NURBS transfer while retaining the command's existing document policy.
[Revolve](../smlib-revolution.md) uses the upstream NURBS tensor constructor and
the same surface transfer and modeling-profile preparation.
[PlanarSrf](../smlib-planar-surfaces.md) delegates plane/contact queries and nested
region construction to usd-brep, excluding the portable containment adapter.
[ExtrudeCrvToPoint](../smlib-apex.md) shares native profile preparation and NURBS
transfer, using ruled surfaces and native base-cap/stitch operations.
[ExtrudeCrvAlongCrv](../smlib-translational-sweep.md) uses the exact translational
sweep constructor and shares native cap construction with apex extrusion.
[Modeling curve imports](../smlib-curve-import.md) now delegate active-interval
clamping to usd-brep's canonical constructor, removing the Rust preparation step.
[Native modeling bases](../smlib-modeling-basis.md) share exact knot preservation
through curve copies and sheet producers, retaining kernel-generated controls.
[Numeric extrusion direction](../smlib-extrusion-direction.md) reuses native
point-set classification, with shared batch policy and active CPlane fallback.
[EdgeSrf](../smlib-edge-surfaces.md) delegates degree/knot synchronization, endpoint
weight normalization, ruled/Coons construction and face topology to usd-brep.
The shared boundary-order policy remains in the geometry crate; native builds
exclude its custom tensor builder from command and oracle construction.
[InsertKnot](../smlib-knot-refinement.md) also delegates curve and tensor refinement
to NLib, preserving full unclamped input definitions and sharing periodic-padding
and outer-knot conventions with the portable backend.
Its [repeated point getter](../commands/insert-knot.md) shares immutable source
ownership, temporary portions, point input and commit policy with InsertKink.
Automatic insertion and local Undo operate on private geometry. Single-face
B-reps delegate exact tensor refinement to the selected backend while retaining
their existing trim topology.
[MakeNonPeriodic](../smlib-nonperiodic.md) shares that native endpoint-split path,
clamping periodic curves and requested tensor directions without the Rust
endpoint control solver in native command or oracle construction.
[ChangeDegree](../smlib-degree-change.md) shares endpoint preparation and target-knot
policy, delegating exact elevation and homogeneous interpolation to NLib for
curves and tensor control rows/columns.
[RemoveKnot](../smlib-knot-removal.md) shares that fitting producer and endpoint
preparation. Curve model-space selection and tensor parameter selection stay
in one geometry policy; native command/oracle edits no longer use its Rust solver.
[RemoveMultiKnot](../smlib-multiple-knot-removal.md) shares group selection and
target-basis metadata, refitting complete groups with the same native producer
in descending parameter order and recomputing V candidates after U edits.

The kernel combines validated finite primitives with compensated arithmetic and
exact fallbacks for difficult binary64 inputs. See [numerical robustness](../numerical-robustness.md)
for implementation boundaries, regression evidence, and focused test commands.
The [convex polyhedral Boolean foundation](../convex-booleans.md) uses exact
half-space clipping and shared edge subdivisions, retaining editable face
surfaces and trims. It replays 26 nonempty public SDK results and records six
set/SDK differences. [BooleanUnion](../commands/boolean-union.md) adapts exact
multiple-operand construction, material/cavity separation, coplanar face merging,
source metadata, selection and history. [BooleanIntersection](../commands/boolean-intersection.md)
adds common-set and two-set policies, with exact pair regions combined before
rounding and 32 native command witnesses. Its [ordered common reducer](../common-intersection.md)
retains exact intermediate regions and follows metadata and success decisions
from 80 additional native recipes. [BooleanDifference](../commands/boolean-difference.md)
subtracts multiple original cutters and applies target, metadata, deletion, and
selection policies from 40 native witnesses. The commands share a two-phase
selection controller with separate option-memory policies.
[BooleanSplit](../commands/boolean-split.md) partitions each target with one exact
original-face arrangement, preserving overlapping cutter interfaces and connected
branch counts for geometry metadata. Unchanged targets keep their identity;
cutters remain in the document. Its two-set getter accepts shared target/cutter
sets. [Finite plane cutters](../boolean-split-plane-cutters.md) add per-region coverage
checks and coplanar sheet groups, with exact component queries retaining cavities
and separating nested islands before export. [Open planar targets](../boolean-split-open-targets.md)
use oriented shared/unshared boundary expressions, complete segment coverage and
an explicit naked-edge exporter while preserving solid closure gates.
[Mixed coplanar stages](../boolean-split-mixed-open.md) carry physical boundary
patches alongside exact region masks, preserving holes and supporting-face
ownership through later cuts. [Trimmed sheets and compound inputs](../boolean-split-topology.md)
separate complete solid caps from open boundary output and keep command shell
participation and geometry metadata outside material-region classification.
[Boolean2Objects](../commands/boolean-two-objects.md) prepares five choices from
original operands, caches readonly preview scenes and accepts in one transaction
while preserving the first source identity and metadata. Its planar-sheet path
uses exact oriented half-spaces and finite physical patches, with separate
coplanar seam and uncut-sheet policies from native commands. Common/exclusive
patch categories supplement original-face lineage so partial overlap seams
survive merging. Explicit unchanged acceptance records its own history step
for measured Invalid result previews. [Planar surface Booleans](../commands/planar-booleans.md)
use finite-area set masks, merge partition seams and retain command-specific
identity, default attributes, empty-result and preselection history policies.
The projection kernel keeps original trim-hole polygons rational through
orthogonal projection and finite-area classification, then extends the first
support only for final validated output. Tilted and collapsed inputs do not
supply rounded intermediate operands. The circular planar adapter certifies
complete disk loops, classifies analytic arc intervals and keeps rational arc
seams, with two native internal-contact discrepancies retained in its evidence.
The mixed planar path splits straight/circular edges, audits input loops and
shares oriented contour assembly for partial arcs and polygonal/circular holes.
Membership checks include model-space distance to avoid surface-domain clamping
errors; completed mixed results can be used as later operands. General curved Boolean commands
are available through the [native SMLib adapter](../smlib-import.md) for qualified
closed solids; arbitrary curved and open inputs remain under implementation.
The [continuous surface/trim certificate](../surface-curve-certificates.md)
now proves complete matched-parameter edge correspondence with exact rational
tensor composition and conservative bounds at knot crossings. Surface cutting
requires these proofs for retained, fitted, and endpoint-adjusted cutting trims.
single-face [control-net edits](../surface-edit-certificates.md) also require them
for every nonrectangular boundary image and shared edge use, with exact collapse
proofs before singular edge removal.
The [fixed-endpoint fitter](../constrained-surface-pullbacks.md) refines inconclusive
proposals while retaining shared topological endpoints. The separate [chained polyhedral Boolean API](../polyhedral-booleans.md)
uses exact plane arrangements and two-sided material classification for concave
faces, holes, cavities, and multiple shells. Model-space certificates admit
bounded UV roundoff in earlier results, while rejecting intersecting input faces
and nonmanifold vertex links. This API is available through the Python oracle;
the [interactive adapters](../polyhedral-boolean-commands.md) now accept those
certificates after the convex path, preserving original-face contributor reports
and captured shell participation policies. [Compound sets](../compound-intersections.md)
and [oriented pairs](../compound-pairs.md) have separate measured pipelines;
arbitrary compound contributor configurations remain uncertified.

[Automatic pullback discovery](../automatic-surface-pullbacks.md) certifies eligible
straight UV branches at seams and collapsed boundaries, reuses exact proof
preparation, and returns its bound without duplicate work.
The [derivative-free fallback](../derivative-free-surface-pullbacks.md) also fits
eligible nonlinear paths through singular nodes, with original-knot-aware
fractional proofs and a final assembled-curve certificate.
[Certified pushup](../certified-surface-pushups.md) constructs spatial images from
UV splines with exact rational composition or adaptive cubic fitting, retaining
native domains and proving the complete normalized-parameter error bound.
[ApplyCrv](../commands/apply-curves.md) maps World-XY curves and points through
selected bounds, with separate source/target picking and atomic document copies.
[CreateUVCrv](../commands/create-uv-curves.md) supplies the reverse workflow with
trim contours, optional spatial inputs and independent group/history handling;
native coarse sizing and off-surface projection have explicit accuracy limits.
Both commands share nonmutating [temporary SubCrv inputs](../uv-subcurve-input.md),
with nested curve/endpoint picking, native default output attributes and parent
command history.
Their [signed-length script inputs](../signed-length-subcurves.md) share a separate
geometry module that extracts anchor-relative intervals before integration.
The [inline numeric getter](../subcurve-length-confirmation.md) separately owns
confirmation, orientation and open-end clamping policies.
The [standalone SubCrv controller](../commands/subcurve.md) shares that command
policy, with separate source picking and native copy/replacement selection.
Its [MarkEnds mode](../subcurve-mark-ends.md) stages default marker points without
copying source attributes or modifying the original curve.
The [FromMidpoint policy](../subcurve-midpoint.md) queries both sides from the
original center, sharing parameter-preserving endpoint recovery with the kernel.
Its [command-specific preferences](../subcurve-option-memory.md) persist accepted
option edits independently of document history and the global Copy switch.
[Direction locking](../subcurve-direction.md) captures viewport hover state for
numeric extraction in standalone and inline getters.
[Closed numeric confirmation](../subcurve-direction-confirmation.md) retains the
source and length in a point getter and chooses between prospective endpoints.
[SubCrv previews](../subcurve-preview.md) cache the derived curve and endpoints
without editing the document, sharing geometry across viewports.
