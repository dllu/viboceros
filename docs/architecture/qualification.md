# Analysis, curve queries and qualification

[Architecture overview](../architecture.md) · [Documentation](../README.md)

Implementation notes for this subsystem. Command pages and qualification records
describe the supported behavior and measured limits.

Tests cover numerical operations, topology, document state, commands, import/export,
and UI interactions. Passing the current suite is evidence for those cases,
not a proof of full Rhino compatibility. The [validation checkpoint](../validation-checkpoint.md)
records a full-workspace and opt-in GPU audit with its exact revision and limits.
The [oracle](../oracle.md) checks public
Rhino outputs and records timing independently of startup and fixture setup.
Its separate [diagnostic replay](../oracle-replay.md) retains ordered per-operation
native successes/errors in one dispatch pass and compares all successful raw
records. Global validation and process failures remain distinct from observations.
The dedicated `brep_interchange` oracle module checks [morphed 3DM exports](../brep-3dm-interchange.md)
through both readers, keeping source fitting, serialization and meshing checks separate.

[B-rep Contour](../contour-brep-parameterization.md) restores face/trim domains,
reversed traversal, closed-loop length parameters and native group thresholds.
A closed-seam scan-line correction prevents extension outside outer trims and
truncation beside holes; 24 new native recipes pass strict replay.

[Edge analysis](../commands/show-edges.md) shares a registry-owned transient session
between console commands, the control window and viewport overlays. Topology is
cached against document geometry snapshots and tolerance; sampled edges share
the viewport display cache. Marked endpoints enter document history as points.
Ten [native SDK captures](../edge-analysis-native.md) qualify edge ordering,
classification, domains and samples for planar trims, boxes and polygon meshes.
Native edge-analysis dialog and zoom parity remain unqualified.

[Native edge marking](../edge-mark.md) now preserves coincident endpoint objects,
sequential and repeated navigation/Mark actions, and command-level Undo/Redo.
Fifty native command workflows match point creation order and source retention;
mixed naked/non-manifold All marking handles B-reps before meshes.

[Edge-analysis input](../edge-analysis-input.md) separates the persistent panel
from transient Zoom sessions, with command-first source gathering, repeated
options, retained marks on Escape and one history group across answers. Both
overlays share a two-entry sampled-edge cache. Sixteen native cancellation
recipes qualify the command API's retained edits on `OperationDeclined`.

[Divide](../commands/divide.md) stages count, length and equal-chord outputs in a
dedicated command module, with current-layer points, source-attribute split
pieces, remainder deletion, per-source point groups and one-step history.
Complete argument lines retain their options through curve picking. The chord
kernel uses analytic primitive paths and exact rational Bernstein sphere
equations for general curves. [Native qualification](../divide-command.md) covers
30 recipes; three circular length cases retain documented numerical differences
at the tighter epsilon. Broader geometry qualification remains open.

[Divide input](../divide-input.md) now gathers curves for a bare command, then
keeps count, length, chord and output edits in a numeric prompt. Cached viewport
station markers share the command's output preparation and retain immutable
source geometry identities. Preview edits leave the document unchanged; Enter
accepts one transaction, while cancellation or source changes discard the
pending preview. Native defaults, seam/direction controls and SubCrv remain open.

[Curved Divide](../divide-curved.md) extends native replay to arcs, ellipses,
rational 3D curves and mixed polycurves. Chord roots retain exact rational
intervals, independently validate denominator poles and distinguish near-contact
complex roots with a bounded Sturm check. Native source inspection corrects
Circle recipe representation; explicit NURBS-circle splits retain their original
rational parameters. Extreme-weight and turnaround Split differences remain
recorded diagnostics.

[Cusp arc length](../arc-length-cusps.md) uses global estimated-error refinement
with exact running sums. A non-dyadic turnaround no longer exhausts a local
depth limit, and signed cancellation respects the global absolute tolerance.
Roundoff-limited intervals leave other refinable intervals active. Eighteen
native Divide recipes now execute; three native cusp inversion differences
remain raw diagnostics.

[Rational line arc length](../rational-line-arc-length.md) measures degree-one
spans from straight geometry and uses exact projective distance/parameter
queries. The sampler retains this metadata through polycurve parameter maps
and bypasses derivative integration and lookup construction for those spans.
Endpoint-sign checks reject poles; direct geometric station sampling retains
points beside heavily compressed native parameters.

The [native surface pull producer](../smlib-surface-pull.md) reuses usd-brep normal
tracing and lifting with caller-controlled tolerances and shared whole-curve
surface image certificates. A generated-source seed repair restores finite
boundary crossings. The [Pull coordinator](../commands/pull.md) shares Project
selection and atomic admission, clips paired witnesses to original face trims,
and removes complete spatial seam duplicates. Mesh curves reuse bounded source
approximation, the shared mesh index and exact triangle Voronoi restrictions;
quadratic distance bounds resolve closest paths across faces, edges and vertices.
Ambiguous discontinuous paths decline before document mutation. See
[mesh Pull](../commands/pull-mesh.md).

Directional `Project` reuses exact affine transforms for planar faces, shared
curve/face clipping, and the existing usd-brep coupled surface intersector for
curved targets. Its ruled source surface retains `C(u)+v*d` as a second source
relation, with original target trims handled by the shared UV certificate path.
The coordinator stages all results before document admission and requires
common-sign rational weights for finite sweep bounds. The app supplies source
and target picking, session options, and finishing-view CPlane/camera directions.
See [Project](../commands/project.md) for supported behavior and qualifications;
SubD targets, grips, and edge subobjects are pending.

[Loose Project/Pull](../commands/projection-loose.md) shares sided Greville
locations and original-knot/weight interpolation. Existing faer collocation and
usd-brep fixed-row/nonuniform periodic solvers produce candidates; exact
rational station bounds admit their requested edit-point positions. Shared
command staging retains one history edit and separate app preferences.

Mesh Project targets reuse a borrowed bounding-volume index, conservative
interval slabs and exact point/triangle admission. The existing shorter quad
diagonal and planar region/triangle conversion feed shared curve clipping.
The shared Join coordinator now accepts an optional source-provenance predicate.
The outward interval arithmetic formerly private to surface certificates moved
to one internal geometry module and is reused by both certificates and mesh
queries. See [mesh projection qualification](../commands/project-mesh.md).
