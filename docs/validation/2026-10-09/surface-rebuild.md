# Surface Rebuild captures

[Historical checkpoint](../../validation-checkpoint.md)

The preceding singular trim Rebuild capture ran twelve recipes on a fresh private
Xvfb scheme. [Complete records](../../../tools/rhino_oracle/observations/surface_rebuild_singular_trim.json)
retain sphere caps, circular holes, half-sphere wedges, a holed cone and swapped/
reversed cap charts. Every native ReTrim=Yes B-rep exactly equals the public
Brep.CreateTrimmedSurface result. Command replay checks degrees/counts/weights,
controls at `1e-6`, knots at `1e-12`, face orientation, source purity, pole/seam
incidence, boundary witnesses at `2e-6` and independent Undo/Redo. Exactly collapsed
natural target sides retain full or partial singular UV intervals and pole vertex
indices without spatial edges. Coherent trim control hulls prove chart containment;
targets that lose their exact collapse are rejected. Analytic tests cover both cap
charts, reversed face sense and perturbed poles. App testing covers readonly cap
preview, acceptance and atomic history.

The first complete replay exposed inward ReTrim=No full-sphere outputs after chart
edits. Newly closed single-face outputs now normalize outward, using the exact
orientation classifier and a numerical signed-volume fallback for an unresolved
connected shell. A second replay exposed a contour-policy difference on the
swapped cap. [Sixty-six independent closest queries](../../../tools/rhino_oracle/observations/surface_rebuild_cap_projection.json)
show about `0.000474` variation in closest U, while native trim transfer keeps U
within `3e-8`. A full constant-U contour on a closed V chart now transfers to the
target isocurve at its projected shared endpoint. The swapped cap retains two edges
like Rhino; normal/reversed caps have 73 local edges versus two native, the sphere
hole 84 versus two, and the cone hole 83 versus three. Extra segments are individually
certified and remain an explicit representation difference. The initial ten-result
artifact is incomplete evidence for its twelve-case request. Both ordinary engine
runners now require the exact requested recipe ID set, rejecting missing or extra
results while allowing reordering. See [guide](../../commands/rebuild-surfaces.md) and
[provenance](../../singular-trim-rebuild-provenance.json). Interior singularities,
multi-chart winding, general solid embedding, broader isocurve-transfer families,
global nearest-point guarantees and performance parity remain unverified.

The preceding trimmed closed-surface Rebuild captures ran eight cylinder patch/hole
recipes and two seam-crossing hole recipes on fresh private Xvfb schemes.
[Patch/hole records](../../../tools/rhino_oracle/observations/surface_rebuild_seam_trim.json)
and [crossing-hole records](../../../tools/rhino_oracle/observations/surface_rebuild_crossing_hole.json)
retain complete source/output B-reps, target controls, edge witnesses, source
purity and independent Undo/Redo. Every native ReTrim=Yes output exactly equals
the public Brep.CreateTrimmedSurface diagnostic. Local replay checks degrees,
control counts and weights, control positions at `1e-6`, scaled knots at `1e-12`,
source purity, loop structure, shared seams and history. When local certification
adds boundary segments, bidirectional physical boundary witnesses use `2e-6`;
the extra segmentation remains an explicit representation difference.
The rectangle has 26 local edges versus four native edges; the interior-hole
face has 59 versus four. These are geometric witnesses, not topology parity.
Per-trim periodic lifts keep both UV uses of a shared seam. Chart-seeded local
projection has bounded global fallback; exact seam sides project in one dimension.
Compact fitting checks additional stations on every original UV span. Unshared
contours can subdivide into individually continuously certified UV/spatial pieces,
with at most eight levels and 256 pieces. Shared seams cannot split independently.
The initial dense-hole case exceeded its surface-image proof budget; the final
bounded subdivision passes without relaxing the correspondence tolerance.
Kernel tests check UV restriction, narrow features missed by ordinary fit stations,
periodic lift selection and cylinder bands. App testing checks readonly band
preview, acceptance and atomic history. The final review added stricter source
and surface-structure assertions and reran the complete retrim replay module.
See [guide](../../commands/rebuild-surfaces.md) and [provenance](../../seam-trim-rebuild-provenance.json).
Generic singular trims, multi-chart winding, global closest-point guarantees,
continuous projection-error bounds and performance parity remain unverified.

The preceding closed surface Rebuild captures ran sixteen default and twelve
varied-degree/count recipes on fresh private Xvfb schemes. [Default records](../../../tools/rhino_oracle/observations/surface_rebuild_closed.json)
and [degree/count records](../../../tools/rhino_oracle/observations/surface_rebuild_closed_degrees.json)
retain spheres, cylinders, cones and tori, corrected swapped charts, a reversed
sphere, full control nets, closure/periodicity/singularity flags, complete B-reps
and independent history. Kernel replay matches public SDK controls at `1e-6`;
command replay checks topology, native edge witnesses at `2e-6`, unchanged sources
and history. Closed directions use cyclic station interpolation with integer
stations for odd degrees and half-integer stations for even degrees. Requested
counts add the degree in stored repeated controls; degree-one output is closed
without periodic classification. Exact constant boundaries remain exact poles.
Natural seam/singular faces retain incidence, orientation and source domains when
retrimming. Analytic tests cover both charts, repeated-control identity, solid
closure and pole rows; app replay checks readonly preview, acceptance and history.
Resource admission counts actual periodic repetitions before solving. The initial
producer discarded returned chart edits; its complete diagnostics remain separate.
See [guide](../../commands/rebuild-surfaces.md) and [provenance](../../closed-surface-rebuild-provenance.json).
Generic singular trim projection, arbitrary periodic sources, degenerate
midpoint isocurves and performance parity remain unfinished or unverified.
TweenSurfaces keeps its preceding measured open preparation policy; these rebuild
records do not establish closed-source tween matching.

The preceding surface Rebuild preview work shares one readonly command preparation
with a cached viewport scene and accepted geometry. Unchanged edits retain scene
identity; deletion/layer changes reuse geometry, and background edits refresh the
scene. Guards validate source geometry, attributes, root text, groups, selection,
tolerance and current layer before edits or acceptance. Failed preparation clears
the scene, blocks acceptance and permits recovery. Tests preserve existing redo,
reject stale queued edits and compare preview/accepted controls with twelve native
outcomes. A [four-command Xvfb capture](../../../tools/rhino_oracle/observations/surface_rebuild_natural.json)
confirms full target boundaries and retained source domains on default 10x10 warped
and curved natural faces. Exact target isocurves preserve reordered edges, reversed
uses and vertex indices while avoiding inverse fitting; straight UV contour images
use canonical parameter speed for direct knot-crossing composition. The first eager
previews exposed work limits/slow inverse fits on those boundaries.
An [eight-stage production UI inspection](../../../tools/rhino_oracle/observations/rebuild_preview_ui.json)
completes replacement/copy previews, an invalid edit, acceptance, Undo/Redo and
cancellation on private Xvfb. Screenshots show two drawn surfaces in a copy preview
while the document layer pane still reports one object. Early UI observers misread
active/invalid command text; the final command-field observer completed successfully.
See [guide](../../commands/rebuild-surfaces.md) and [provenance](../../rebuild-preview-provenance.json).
Curve previews, seam/singular retrimming, split natural-side specialization, native
pixel appearance, general closest-point guarantees and performance parity remain
unfinished or unverified.

The preceding surface Rebuild retrim captures ran fourteen owned native commands
and two public trim-transfer queries on private Xvfb. [Initial records](../../../tools/rhino_oracle/observations/surface_rebuild_retrim.json)
and [follow-up records](../../../tools/rhino_oracle/observations/surface_rebuild_retrim_followup.json)
retain complete source/output topology, controls, 33 edge stations, 81 surface
stations and independent history for planar holes, nonuniform parameterization,
warped cuts, curved/rational sources and natural boundaries. Every native
ReTrim=Yes outcome equals public Brep.CreateTrimmedSurface and retains source UV
domains; UV copying disagrees materially on the nonuniform source. Local command
replay checks controls at `1e-6`, native edge distance witnesses at `2e-6`, source
purity and history. A [two-query API capture](../../../tools/rhino_oracle/observations/brep_retrim_geometry.json)
projects a paraboloid annulus onto parallel planes. API replay checks loops,
vertices and edge witnesses at `2e-6`. App and analytic regressions check holes,
area, orientation, readonly edits and trim/edge certificates. A default 10x10
circular-hole regression initially exhausted the general knot-crossing proof;
an exact affine control-net reference certificate now proves that case while
retaining rational image controls. Incorrect translated images and mixed-sign
UV proposals are rejected. See [guide](../../commands/rebuild-surfaces.md),
[certificate](../../surface-curve-certificates.md) and [provenance](../../surface-rebuild-retrim-provenance.json).
Projection searches and fits have sampled accuracy checks; new spatial edges
are continuously certified against their new UV trims. Global closest-point
projection, seam/singular trims, native previews and performance parity remain
unfinished or unverified.

The preceding surface Rebuild option captures ran a 25-step preference sequence and
nine-step ordered degree/count follow-up on fresh private Xvfb schemes.
[Primary prompts](../../../tools/rhino_oracle/observations/surface_rebuild_options.json)
and [ordered follow-up](../../../tools/rhino_oracle/observations/surface_rebuild_option_followup.json)
show immediate persistence of every valid option edit through cancellation and
Undo/Redo. Scripted degree growth raises the count; a count below the current
degree minimum is rejected. App replay compares all 34 initial/final states,
source purity, cancellations, accepted control nets and independent history.
Numeric value questions, inline versus picked invocation, unchanged invalid
edits and new-registry isolation have separate regressions. Shared command-owned
parsing preserves token order and keeps preferences outside document history.
See [option provenance](../../surface-rebuild-options-provenance.json). Native dialog
coupling, restart persistence, curve/surface preference interaction, previews and
trim projection remain unfinished or unverified.

The preceding surface Rebuild captures ran twelve owned native commands and twelve
public SDK queries on private Xvfb. [Command records](../../../tools/rhino_oracle/observations/surface_rebuild.json)
retain complete surfaces, 81 stations, attributes/groups, source identity and
independent history for polynomial/rational inputs and all copy/replacement and
input/current-layer combinations. Native command geometry exactly equals public
SDK results. Local replay uses `1e-6` for controls and exact knots/weights.
The [standalone API audit](../../../tools/rhino_oracle/observations/surface_rebuild_local_audit.json)
matches all twelve asymmetric/reduced target structures, with maximum coordinate
error about `2.2e-7` and zero local failures. Its initial audit exposed two rational
arc-length endpoint overshoots by one binary64 step; endpoint parameters now use
exact domain boundaries. Shared rebuilding retains the preceding TweenSurfaces
regressions. App checks command-first picks, inline options, replacement and one
Undo. Command regressions reject unsupported retrimming and invalid options
without partial edits, and check explicit `ReTrim=No`. See
[guide](../../commands/rebuild-surfaces.md) and [provenance](../../surface-rebuild-provenance.json).
General retrimming, periodic/singular inputs, mixed curve/surface batches, native
option persistence and performance parity remain unfinished or unverified.
