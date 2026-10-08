# Validation checkpoint

[Architecture and status](architecture.md) · [Rhino oracle](oracle.md)

This is a reproducible regression checkpoint, not a compatibility certificate.
The October 8, 2026 audit tested code at `1ad4cda7` with Rust 1.95.0 after
[Matched Refit surface tween construction](commands/tween-surfaces.md).

## Commands and results

```sh
cargo test --workspace --release
python3 -m unittest discover -s tools/rhino_oracle -t .
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
git diff --check
```

All commands completed successfully. The final release workspace suite passed
5,281 tests:

| Package | Passed | Ignored |
| --- | ---: | ---: |
| App | 1,003 | 17 |
| Command | 1,187 | 2 |
| Document | 186 | 5 |
| Drafting | 159 | 6 |
| Geometry | 2,064 | 8 |
| I/O | 195 | 0 |
| Oracle | 487 | 0 |

The Python suite passed 965 tests. The 38 ordinarily ignored Rust tests were
not run in this checkpoint. The September 12 audit of `bd299074` separately
passed six opt-in GPU tests covering 182 renders on NVIDIA GB10 / Vulkan /
driver 610.43.02; that is historical evidence, not a new graphics check.
See [GPU tests](gpu-tests.md) for their pixel assertions and limits.

The newest Refit capture ran 24 owned public commands on private Xvfb under
`VibocerosOracleTweenRefit20261008`. All succeed with the requested output count.
[Complete records](../tools/rhino_oracle/observations/tween_surfaces_refit.json)
retain option-initialization diagnostics separately from later bare acceptance,
with original nets, 81 stations per surface, identity, attributes/groups and
independent history. Kernel replay checks 24 accepted nets plus six earlier
requested Refit nets at `1e-7` for controls and `1e-12` for knots/weights. Command
replay checks every source/output with 81 UV-normalized witnesses at `1e-7`,
identity, metadata/groups and history. Refit uses degree elevation and common
knot refinement across unequal positive rational nets, retaining the end source
UV domains. Independent polynomial tests verify normalized parameter agreement
at `1e-12` across different degrees, U/V knot sites and multiplicities while
preserving sources. App tests cover Refit selection, unequal degrees and history.
Native option initialization creates extra objects; bare acceptance does not.
Unequal-net control matching, native option memory/previews, general adaptive
fitting parity and relative performance remain unfinished or unverified. See
[workflow](commands/tween-surfaces.md) and [provenance](tween-refit-provenance.json).

The preceding sampled-tween capture ran 20 owned public commands on private Xvfb
under `VibocerosOracleTweenSampling20261008`. All succeed with exactly the requested
output count. [Complete records](../tools/rhino_oracle/observations/tween_surfaces_sampling.json)
retain sample counts 2, 3 and 6 for planar, warped/rational and unequal-degree/
count/domain inputs, plus source-layer outputs. Every command output definition
is identical to its public sampling SDK result. Kernel replay compares controls
at `1e-7` and knots at `1e-8`, also replaying all 22 earlier sampling SDK outputs.
Command replay checks 81 UV-normalized witnesses per source/output at `1e-7`,
source identity, attributes/groups and independent history. Sampled tensor
interpolation blends normalized source UV grids and uses mean-chord output
knots/domains; initial command defaults follow captured SamplePoints/10 settings.
The Python/JSON oracle exposes `surface_tween_sampled_geometry` with full output
nets. Independent tests require analytic plane agreement at `1e-12`, retain
source purity and reject excess work before allocation. App regression checks
method/sample/count options through source picking and history. Sampling is
bounded at 255 divisions per axis and a million aggregate controls. Unequal-net
control matching, Refit, remembered options and corner previews remain unfinished;
sampled grid interpolation is not a continuous exact source blend or native
boundary certificate. See [workflow](commands/tween-surfaces.md) and
[provenance](tween-sampling-provenance.json).

The preceding surface-tween capture ran 22 owned public commands on private Xvfb
under `VibocerosOracleTweenSurfacesVerified20261008`. All succeed natively.
[Complete records](../tools/rhino_oracle/observations/tween_surfaces_command.json)
retain full surface nets, 81 stations per object, source identity, properties,
groups, command events and independent history, plus public sampling SDK data.
The new `TweenSurfaces` command implements compatible control-net matching,
explicit direction changes and output-layer attributes/groups, with staged
atomic output. Eight native compatible recipes replay controls at `1e-6` and
knots/weights at `1e-12`, including rational displacement via square-root weight
ratios. Independent kernel tests cover finite extreme coordinates, source purity
and resource rejection; command/app tests cover groups and history. Unequal-net
common-chart fitting, Refit, SamplePoints, native default/option memory and corner
previews remain unfinished. Refit leaves four source-shaped native copies and
SamplePoints two; these are retained diagnostics, not local parity claims.
See [workflow and limitations](commands/tween-surfaces.md) and
[provenance](tween-surfaces-provenance.json).

The preceding circular scale capture ran 27 owned public commands on private Xvfb
under `VibocerosOracleCircleScale20261008`. All succeed natively and locally.
[Complete records](../tools/rhino_oracle/observations/planar_circle_scale.json)
retain equal/unequal radii, swapped operands, full edge curves, stations,
identity, metadata and independent history at scales `2^-20`, `1` and `2^20`.
All local normalized analytic areas agree at `1e-9`. Six equal-radius cases
at scales `1` and `2^20` meet the existing normalized bidirectional boundary
bound `5e-6`; the other 21 retain individually measured discrepancies. Unequal
radius native fits differ by up to `8.632e-6` at those scales; small-scale
bidirectional witnesses differ by up to `0.004588` and normalized native areas
by up to `0.014638`. Tests reproduce these diagnostics instead of weakening
the agreement bound. All cases check counts, identity, metadata and history;
app replay checks all picking/history workflows. Shared exact circle-pair
coefficients replace duplicated squared-radius subtraction. An independent
172-case Fraction/180-digit Decimal reference verifies bit-exact longitudinal
offsets and height within one binary64 step, including primitive exponents
`-550` through `600` and near-internal contacts. A regression removes an old
`6.9e-4` height error. Center-distance and angle arithmetic remain binary64;
whole-model extreme scaling, continuous native boundary agreement, general
curved Booleans and relative performance remain unverified. See
[arithmetic](planar-cut-numerics.md) and [provenance](planar-circle-scale-provenance.json).

The preceding planar scale capture ran 27 owned public recipes on private Xvfb
under `VibocerosOraclePlanarScale20261008`. All succeed natively and locally.
[Complete records](../tools/rhino_oracle/observations/planar_boolean_scale.json)
retain curves, samples, topology, identity, metadata and independent history
at scales `2^-20`, `1` and `2^20`. Command replay checks bidirectional finite
curve witnesses at `5e-6 * scale`; local normalized total areas match analytic
formulas at `1e-9`. Native normalized output areas differ by up to `2.103e-4`,
and source areas by up to `1.222e-4`. Only this capture uses normalized native
area bounds `3e-4` and `2e-4`; earlier tolerances stay unchanged. App replay
checks all 27 picking/history workflows. Exact rational cut coefficients fix
cancelled, overflowing and underflowing line determinants and stabilize
line/circle roots; 206 independent Python Fraction cases check line stations
bit for bit. Boolean export reconciles independently rounded junction endpoints
within the document tolerance while preserving general PolyCurve coincidence
rules. Primitive regressions include exponents `-550` through `600`, exact
endpoint roots, unrepresentable cuts and rejection of excessive junction gaps.
[Arithmetic details](planar-cut-numerics.md) and [provenance](planar-boolean-scale-provenance.json)
retain the evidence and limits. Whole-model extreme scaling, continuous native
boundary certificates, general curved Booleans and relative performance remain
unverified.

The preceding mixed planar capture ran 34 owned public recipes on private Xvfb under
`VibocerosOraclePlanarMixedVerified20261008`. All succeed. [Complete records](../tools/rhino_oracle/observations/planar_boolean_mixed.json)
retain full NURBS edge definitions, 33 witnesses per edge, areas, counts, identity,
metadata and independent Undo/Redo. [Provenance](planar-boolean-mixed-provenance.json)
binds the capture and the measured partial-arc endpoint shift. Command replay
checks every outcome with curve witnesses at `5e-6` (partial-arc reverse witnesses
at `1e-5` after measuring a `6.7e-6` native endpoint shift), area at `2e-5`, topology,
identity, metadata and history. Earlier capture tolerances are unchanged. Newly
created components are paired by curve geometry and original identity; insertion
order can differ. Replays cover disk/rectangle cuts in both orders, narrow strips,
containment, disjoint/tangent regions, offset planes, polygonal/circular holes,
partial input arcs and three-input Union. App replay checks all 34 getter/history
workflows. The kernel splits recognized straight/circular boundaries and shares
oriented contour assembly, preserving arcs, corners and seams. Input loop audits
reject crossings/overlap; monotone same-sign straight controls, finite derived
arithmetic and bounded work protect preparation. Membership also checks spatial
distance so clamped surface parameters cannot admit outside points. Independent
tests compare analytic half-disk area at `1e-9`, split strips, chain a mixed result
as a later operand and verify self-intersection/work rejection. A production
wgpu/egui inspection on private Xvfb checks Circle/PlanarSrf and SrfPt sources,
ordered disk/rectangle Difference, preselected mixed Union, Ghosted mode and
Undo/Redo. See [usage and saved image](commands/planar-booleans.md). General
splines, nonparallel circular projection, compound surfaces, broader coincident
junctions, near contacts, arbitrary insertion order, native pixels, restart
behavior and relative performance remain unsupported or unverified.

The preceding circular planar capture ran 28 owned public recipes on private Xvfb
under `VibocerosOraclePlanarCircularVerified20261007`. All succeed natively.
[Complete records](../tools/rhino_oracle/observations/planar_boolean_circular.json)
retain full NURBS edge definitions and 33 samples per edge, alongside areas,
counts, identity, metadata and independent history. [Provenance](planar-boolean-circular-provenance.json)
binds all records and two explicit internal-contact diagnostics. All 28 local
outcomes replay metadata/history; 26 match native topology and bidirectional
finite curve witnesses at `5e-6`. Area comparisons use `2e-5` for native mass
integration and perturbed projected arcs; polygon tolerances remain unchanged.
The independent kernel tests require analytic area at `1e-9` and circular locus
stations at `1e-12`, with containment/holes, disconnected tangency, three-input
Union and duplicate input coverage. Whole-span circular-locus and simple-loop
certificates admit complete disk boundaries; analytic arc intervals carry region
boundaries into rational NURBS output. Finite term accumulation determines loop
orientation without summation loss, and preparation/export work is bounded.
App replay covers all 28 selection and history workflows. Native internal-tangent
Difference removes the exact contact point with a reverse witness gap between
`0.005` and `0.006`, while local output retains it. Native Intersection creates
three perturbed edges where local output has one circle; its boundary witnesses
stay within `5e-6`. Both discrepancies remain explicit and do not establish
native parity. Production wgpu/egui inspection on private Xvfb checks Circle/
PlanarSrf inputs, circular Difference/Union, ordered picks, Ghosted mode and
Undo/Redo. See [usage and saved image](commands/planar-booleans.md). The mixed extension above
adds the captured line/arc and hole combinations. General curved loops,
nonparallel circular projection, arbitrary coincident junctions, near contacts, native pixels,
restart behavior and relative performance remain unsupported or unverified.

The preceding trim-hole/projection capture ran 31 owned public recipes on private Xvfb
under `VibocerosOraclePlanarTopologyVerified20261007`. All succeed.
[Complete records](../tools/rhino_oracle/observations/planar_boolean_topology.json)
measure hole fill/cover/straddle/cut regions, holed cutters, two different hole
boundaries, tilted/reversed inputs, a tilted first support, perpendicular collapse
and multi-input hole filling. [Provenance](planar-boolean-topology-provenance.json)
binds the producer, recipes and complete capture. Command replay compares every
boundary bidirectionally at `1e-7`, scalars at `1e-9`, topology, identity, metadata
and independent history. App replay runs every source-picking phase and checks
area, counts and Undo/Redo. The separate geometry module projects original
physical polygons rationally before arrangement classification; only final
export rounds, extending the first support as needed. Independent kernel tests
verify both projection orders, output plane membership, perpendicular zero area,
source purity, warped-input rejection and work limits. The second planar getter
excludes its already-picked source before hit ranking, so nested cutters remain
pickable in Shaded/Ghosted; a regression checks both modes. Production wgpu/egui
inspection on private Xvfb checks hole creation, a tilted cutter yielding two
pieces, Ghosted mode, ordered clicks and Undo/Redo. See [usage and saved image](commands/planar-booleans.md).
Curved trims, compound surfaces, broader hole combinations, near contacts,
arbitrary insertion order, restart behavior, native pixels and relative
performance remain unsupported or unverified.

The preceding planar-surface Boolean capture ran 28 owned public recipes on private
Xvfb under `VibocerosOraclePlanarBooleanCorrected20261007`. All succeed.
[Complete records](../tools/rhino_oracle/observations/planar_boolean_command.json)
measure all three commands for overlap, reversed normals, containment in either
order, equality, separated/touching sheets, parallel offset sheets, three-input
Union and preselection. [Provenance](planar-boolean-provenance.json) binds the
producer, recipes and complete capture. Command replay compares boundaries in
both directions at `1e-7`, scalars at `1e-9`, topology, identity, default/source
attributes, groups, geometry-root text and independent Undo/Redo. The app replay
runs all recipes through native selection/admission phases and compares area,
topology, preselection Undo state and history. The finite-area set API shares one
original arrangement for multiple inputs, excluding planning rectangles; the
kernel test covers opposite normals, multi-input area and work/output limits.
Union/Difference create default-attribute surfaces on the current layer;
Intersection retains the first ID, attributes and groups while clearing geometry
user text. Empty Difference/Intersection results delete both inputs successfully.
The production wgpu/egui inspection on private Xvfb checks ordered Difference/
Intersection picks, three-surface preselected Union, Ghosted mode and Undo/Redo.
See [usage and saved image](commands/planar-booleans.md). The projection/hole
extension above adds those measured cases. Curved boundaries, compound surfaces,
broader trim holes, near contacts,
restart behavior, native pixels and relative performance remain unverified.

The preceding coplanar Boolean2Objects capture ran 37 owned public recipes on private
Xvfb under `VibocerosOracleBooleanTwoCoplanarVerified20261007`. Thirty-six succeed
and one cancels. [Complete records](../tools/rhino_oracle/observations/boolean_two_coplanar.json)
retain all five modes for partial overlap, opposite normals, smaller-first nested
sheets, equality, opposite equality, disjoint sheets and edge contact, plus
retention and cancellation. [Provenance](boolean-two-coplanar-provenance.json)
binds the producer, bounded real-click driver, recipes and entire capture.
Command replay checks complete geometry/counts, boundary witnesses in both
directions at `1e-7`, scalars at `1e-9`, mass at `1e-10`, metadata, identity and
independent Undo/Redo. App replay checks original cycle counts and source/history
purity. The kernel retains common/exclusive patch categories separately from
supporting-face lineage; merging preserves shared trim seams. Either Difference
uses both exclusive regions, with multi-piece geometry-root text cleared.
Accepted Invalid result previews preserve both sources but create an explicit
unchanged Undo entry; the document regression tests rollback and Redo retention.
A production wgpu/egui inspection on private Xvfb checks partial-overlap Difference
and identical-sheet Invalid result previews, Enter, Undo/Redo and Ghosted mode.
See [usage and saved images](commands/boolean-two-objects.md). General compound,
trimmed-hole combinations, curved inputs, near contacts, insertion order,
restart persistence, native pending pixel parity and relative performance remain
unverified.

The preceding planar Boolean2Objects follow-up ran 48 owned public recipes across two
private Xvfb sessions. Forty-six succeed, one partial crossing fails and one
cancels. [Combined replay](../tools/rhino_oracle/observations/boolean_two_open.json)
retains every outcome; unchanged raw sessions and exact assembly are bound by
[provenance](boolean-two-open-provenance.json). Replays compare all five modes
for plane/box in either order, reversed normals, perpendicular sheets, nested
coplanar sheets, uncut contained/disjoint sheets, parallel sheets, retention,
cancellation and independent Undo/Redo. Command checks include bidirectional
boundary witnesses at `1e-7`, scalars at `1e-9` and volume/centroid at `1e-10`,
identity, attributes/groups and geometry-root metadata. App checks use original
click counts and verify untouched sources/history before acceptance. Independent
kernel tests check finite section coverage, normal-relative half-box mass,
coplanar area/holes and plan isolation. The mixed plan exports only physical
patches while retaining one exact original arrangement. A production wgpu/egui
inspection on private Xvfb checked a typed SrfPt/Box pair, preselection, four real
cycle clicks, Enter, Undo/Redo and Ghosted rendering of closed and open outputs.
See [usage and saved image](commands/boolean-two-objects.md). Curved/nonplanar open
inputs, arbitrary compound and trimmed-hole policies, near contacts,
restart persistence, native preview pixel parity and relative performance remain
unsupported or unverified.

The preceding closed-solid Boolean2Objects capture ran eleven owned public
recipes on private Xvfb under `VibocerosOracleBooleanTwoVerified20261007`. Seven succeed, three fail before
cycling and one cancels. [Raw records](../tools/rhino_oracle/observations/boolean_two_command.json)
retain bounded click counts, cycle prompts, final boundaries, mass properties,
metadata, identity, idle selection and independent Undo/Redo. Replays check every
completed outcome with bidirectional boundary witnesses at `1e-7`, scalars at
`1e-9` and volume/centroid at `1e-10`. All five choices share one exact arrangement;
preview scene caches leave source geometry/history untouched and acceptance
creates one transaction. A production wgpu/egui inspection on private Xvfb
checked picking, click cycling, Escape, Enter, Undo/Redo and Ghosted mode changes
while pending. See the [saved image, scope and provenance](commands/boolean-two-objects.md).
That checkpoint supported certified closed polyhedral inputs. The planar
extension above adds the measured open-sheet cases. Curved inputs, arbitrary
compound policy, near contacts, restart option persistence, native preview pixel
parity and relative performance remain unsupported or unverified.

The preceding topology BooleanSplit capture ran 14 owned public recipes on private
Xvfb under `VibocerosOracleBooleanSplitTopologyPilot20261007`. Twelve succeed and
create 33 pieces; two failures remain in the [raw records](../tools/rhino_oracle/observations/boolean_split_topology.json).
Replay checks every outcome, face/edge counts, boundary witnesses at `1e-7`,
scalars at `1e-9`, solid volume/centroid at `1e-10`, attributes/groups, geometry
user text, idle selection and independent Undo/Redo. Compound targets/cutters,
original trim holes, missing cap patches, coplanar cover/straddle behavior,
preselection and retention are measured. The command omits untouched closed
shells from successful outputs while restoring the full original on Undo.
Trimmed-sheet boundary splitting distinguishes outer coverage from enclosed holes;
the strict solid-partition API retains its complete-cap contract. Open reports
carry branch component counts for metadata and repeat untouched open components
where the capture requires them. Kernel/app tests verify these distinctions,
original-hole remainders and history. Earlier BooleanSplit capture replays also
pass. See [scope and provenance](boolean-split-topology.md). General curved and
higher-degree geometry, overlapping shells, broader compound/trim combinations,
near contacts, insertion order and relative performance remain unverified.

The preceding mixed-stage BooleanSplit capture ran 20 owned public recipes on private
Xvfb under `VibocerosOracleBooleanSplitMixedOpenVerified20261007`. All succeed and
create 56 pieces. [Raw records](../tools/rhino_oracle/observations/boolean_split_mixed_open.json)
retain boundaries, counts, mass properties, metadata, groups, idle selection and
independent Undo/Redo. Replay checks every outcome, boundary witnesses at `1e-7`,
scalars at `1e-9`, solid volume/centroid at `1e-10`, supporting-face partitions and
history. Coplanar stages before/after sheets and solids, normal reversal,
contained/straddling/full/disjoint overlaps, hole coverage, preselection and
retention are measured. The kernel carries physical boundary patches alongside
exact region expressions and exports only after all stages. Kernel/app tests
verify order, hole decisions, ownership, source isolation, pending purity and
one history entry. A fresh production wgpu/egui inspection on private Xvfb
confirms target picking, ordered typed cutters, three pieces with two retained
cutters, idle selection and Undo/Redo in Ghosted mode. The [image and scope](boolean-split-mixed-open.md)
distinguish that local inspection from native pixel parity. General curved and
higher-degree geometry, arbitrary trimmed topology, ambiguous junctions,
compound policy, near contacts, insertion order and relative performance remain
unverified.

The preceding open-target BooleanSplit capture ran 16 public recipes on private Xvfb under `VibocerosOracleBooleanSplitOpenPilot20261007`. Eleven succeed and create
27 pieces; five failures remain in the [raw records](../tools/rhino_oracle/observations/boolean_split_open.json).
Replay covers all outcomes, face/edge counts, bidirectional boundary witnesses
at `1e-7`, scalars at `1e-9`, solid volume/centroid at `1e-10`, target attributes,
groups, geometry user text, idle selection and independent Undo/Redo. Plane/box
results contain a closed half-box and an open joined boundary; target reversal
chooses the opposite half. Plane/plane results contain cutter faces, and
coplanar overlap retains the shared supporting face. The new open exporter
assigns Boundary trims to naked edges while the solid exporter retains closure
checks. Complete segment coverage uses exact interval unions. Kernel/app tests
check orientation, finite coverage, isolation of the closed-target contract,
picking, retention, cancellation and history. A fresh production wgpu/egui
inspection on private Xvfb confirms the surface-as-target workflow, retained
cutter and Undo/Redo in Ghosted mode. See the [saved image and scope](boolean-split-open-targets.md).
That checkpoint rejected mixed coplanar/noncoplanar stages; the boundary-patch
extension above adds the recorded combinations. Curved/nonplanar inputs,
higher-degree supports, broader trimmed/compound policies, tolerance contacts,
native insertion order and relative performance remain unverified.

The preceding finite-plane BooleanSplit capture ran 19 public recipes on private Xvfb under `VibocerosOracleBooleanSplitPlanesPilot20261007`. Fourteen commands succeed
and produce 39 closed pieces; five no-split failures remain in the
[raw records](../tools/rhino_oracle/observations/boolean_split_plane.json).
Replay covers all outcomes, face/edge counts, bidirectional boundary witnesses at
`1e-7`, scalar fields at `1e-9`, volume/centroid at `1e-10`, attributes, groups,
geometry-user-text lineage, immutable cutter snapshots, idle selection and
independent Undo/Redo. Complete, partial, exact, joint and gapped coverage,
normal reversal, diagonal planes, cutter order and mixed solid/surface stages
are measured. Coplanar sheets can cover a section jointly; an earlier full cut
can make a later partial sheet effective on one connected piece. Planning cells
outside actual sheets cannot provide result faces. Exact material-component
queries preserve cavity shells and separate nested islands before any export.
Kernel tests verify finite coverage, source-face ownership, normal-relative side
reports, nested material, work limits and warped-sheet rejection. App tests and
a fresh production wgpu/egui check on private Xvfb confirm both source types,
viewport picking, acceptance, failed partial cuts, source purity and Undo/Redo.
The [saved image and scope](boolean-split-plane-cutters.md) distinguish that
local inspection from native pixel comparison. Curved/nonplanar sheets,
higher-degree supports and broader trimmed/compound policies remain unverified.
That checkpoint did not support open targets; the planar extension above adds
their measured shared-boundary cases. Tolerance contacts, interleaved coplanar
groups and relative performance still need evidence.

The preceding BooleanSplit capture ran 28 owned public recipes on private Xvfb under
`VibocerosOracleBooleanSplitVerified20261007`. Seventeen commands succeed and
create 64 pieces; eight no-split failures and three cancellations remain in the
[raw records](../tools/rhino_oracle/observations/boolean_split_command.json).
Two follow-ups verify DeleteInput memory after cancellation in either getter.
Command replay covers all 25 completed/failed outcomes, face/edge counts,
bidirectional boundary witnesses at `1e-7`, other scalars at `1e-9`, volume and
centroid at `1e-10`, source attributes/groups, geometry user text, unchanged
object identity and independent Undo/Redo. EndCommand still reports selected
cutters; completed idle snapshots release them. The implementation follows the
idle states. Kernel tests cover every crossed-cutter region, original-face
ownership, duplicate cutters, empty cutters, nested material, concave faces,
holes, cavities, disjoint shells and volume conservation. All target regions
and connected ancestry share one original-face arrangement; rounded intermediate
B-reps never become operands. App tests and a fresh production wgpu/egui
inspection on private Xvfb verify both selection phases, shared sets,
preselection, retention, cancellation, acceptance and Undo/Redo. See the
[saved image, scope and provenance](commands/boolean-split.md). Native insertion
order differs. That checkpoint did not support open cutters; the finite-plane
extension above adds their certified planar case. Curved cutters, broader
compound command policies and relative performance remain unverified.

The preceding edge SubCrv capture ran eleven owned public commands on private Xvfb
under `VibocerosOracleSubcurveEdgesFinal20261007`, including box, planar surface
and circular cylinder edges. All succeed. [Complete native records](../tools/rhino_oracle/observations/subcurve_edge.json)
retain parent B-rep definitions, public edge preselection, macros, events,
metadata and independent Undo/Redo. Application replay checks 297 curve stations,
directed endpoints and four marker points at `1e-6`, copied attributes/groups,
source snapshot identity and complete history restoration. Copy=No retains the
parent and creates a new curve. Tests cover typed edge references, atomic
failures, stale sources, numbered ambiguity choices and cancellation. A real
CPU pointer-event test covers both ordinary curve clicks and surface edges;
a cache regression checks reuse across moving hover and invalidation by edge,
source and tolerance. The final application rerun includes that cache test.
A fresh production wgpu/egui inspection on private Xvfb confirms edge selection,
curve/marker previews, acceptance and Undo/Redo. The [saved screenshot and scope](subcurve-edge-input.md)
document that inspection. Native command-first mouse picking, numeric edge
getters and arbitrary trimmed-edge parity remain unverified.

The preceding SubCrv preview checks ten pending workflows against saved native
confirmed results: 330 stations and directed endpoints at `1e-6`. Repeated
preview calls preserve source snapshot identity, attributes, selection and
history, reuse cached geometry, and cover Copy, MarkEnds, locked-side rejection,
midpoint symmetry and cancellation. The CPU egui check covers four viewport
orientations in Wireframe, Shaded and Ghosted, including no hovered viewport and
inactive input. A fresh private-Xvfb application inspection through the production
wgpu/egui renderer confirms trim overlays, marker-only mode and removal after
Escape. The [saved image and scope](subcurve-preview.md) document that inspection;
this does not add a new opt-in GPU raster test or a native preview measurement.

A preceding 32-recipe closed SubCrv numeric workflow ran on private Xvfb under
`VibocerosOracleClosedDirectionComplete20261007`. The
[raw records](../tools/rhino_oracle/observations/subcurve_direction_grid.json)
retain 16 intermediate snapshots with public InGetPoint=true and
InGetObject=false, intact sources and no created geometry. The generic Select
curve prompt keeps a pending number; it does not start a new source selection.
Twenty-six commands succeed and six cancel an unconfirmed length. Application
replay checks original input tokens, pending source/start/length, released source
selection and absent history, then 924 output curve stations and endpoints at
`1e-6`, selection and independent Undo/Redo states. Opposite-side confirmation
can override the original lock; a skewed polyline distinguishes nearest candidate
endpoints from the shorter arc toward the pick. Two nested UV workflows preserve
parent-owned history. Explicit script regressions retain valid closed lengths
in both directions. See [scope and provenance](subcurve-direction-confirmation.md).

A preceding 18-recipe SubCrv direction workflow ran on private Xvfb under
`VibocerosOracleDirectionVerified20261007`. The
[raw records](../tools/rhino_oracle/observations/subcurve_direction.json)
retain actual motion, locked/unlocked getter prompts, every public
SendKeystrokes input, command events, source/output definitions and independent
Undo/Redo states. Ten commands succeed; four opposite-side open point inputs
end with Failure and four closed numeric inputs with Cancel. App replay checks
330 output curve stations and two markers at `1e-6`, directed endpoints, source
purity, selection and history. The backward seam-crossing polyline numeric case
succeeds. The four cancelled closed numbers remained in a point getter; the
follow-up above corrects the earlier interpretation as a source restart. Inline
open numeric input shares the policy, with source purity and parent-owned history. A CPU
viewport test confirms that hover reports a location without accepting a point
or editing history. See [scope and provenance](subcurve-direction.md).

A preceding 37-step SubCrv preference workflow ran on private Xvfb in settings
scheme `VibocerosOracleSubcurvePreferenceFinal20261007`. The
[raw records](../tools/rhino_oracle/observations/subcurve_preferences.json)
retain default queries, accepted/cancelled option changes, completed geometry,
global Remember toggles, command events and complete source/result snapshots.
Copy, Mode and FromMidpoint persist immediately, even after cancellation;
SubCrv Copy is not reset by the global RememberCopyOptions switch in this capture.
Hidden Mode prompt fields remain absent. Copy=No queries and completed MarkEnds
steps independently establish their saved value. Application replay checks all
37 steps, visible defaults, loci/marker points at `1e-6`, identity and selection.
Command tests verify omitted values, invalid syntax, registry isolation and
document-history independence. Preferences remain session-only; restart
persistence, direction/edge lifetimes and complete native failure timing are
unverified. See [scope](subcurve-option-memory.md) and
[provenance](subcurve-option-memory-provenance.json).

The preceding 22-recipe FromMidpoint capture ran on private Xvfb with settings scheme
`VibocerosOracleSubcurveMidpointFinal20261007`. Nineteen commands succeed; zero
and half/full-perimeter coincident endpoints remain native cancellations.
The [raw records](../tools/rhino_oracle/observations/subcurve_midpoint.json)
retain original/output curves, stations, marker points, metadata, groups,
selection, events and Undo/Redo. Numeric inputs are half-lengths and finish
immediately; point inputs derive that radius along the source. Open sides clamp
independently, while closed endpoints wrap and keep their forward interval,
including the over-half-perimeter remainder. Command/app replays compare every
locus, directed endpoint and marker at `1e-6`, with metadata and history checks.
They cover curved input, Copy/replacement, endpoint centers, option changes and
MarkEnds. Midpoint policy inherits numerical integration and parameter-resolution
limits; direction locking, live preview, B-rep edges, restart option persistence and
reactive History remain outstanding. See [scope](subcurve-midpoint.md) and
[provenance](subcurve-midpoint-provenance.json).

The preceding 15-recipe SubCrv MarkEnds capture ran on private Xvfb with settings
scheme `VibocerosOracleMarkEnds20261007`. Thirteen commands produce 26 endpoint
markers; zero and missing confirmation remain native cancellations. The
[raw records](../tools/rhino_oracle/observations/subcurve_mark_ends.json) retain
full original curves, point coordinates, attributes, groups, selection, command
events and Undo/Redo. Command and app tests replay all marker coordinates at
`1e-6`, with source purity and default-attribute checks. Both Copy choices produce
ungrouped, unselected current-layer points; a full closed traversal retains two
coincident markers. The app can change Mode during input without edits, and bad
modes or geometry fail atomically. Undo/Redo restore marker IDs and empty
selection. Restart mode persistence and reactive History remain unimplemented; see
[scope](subcurve-mark-ends.md) and [provenance](subcurve-mark-ends-provenance.json).

The preceding 17-recipe standalone SubCrv capture ran on private Xvfb with settings
scheme `VibocerosOracleStandaloneSubcurveFinal20261007`. Fifteen commands succeed;
zero and missing confirmation remain native cancellations. The
[raw records](../tools/rhino_oracle/observations/standalone_subcurve.json) retain
full source/output definitions, 33 stations per curve, names, layers, user text,
groups, selection, events and Undo/Redo. Command and application replays compare
every locus and directed endpoint at `1e-6`, with metadata and history checks.
The app additionally accepts starting without preselection and choosing a source
by viewport or ID. Copy results use the original layer/groups and are selected;
replacement retains identity and clears selection. Standalone open point picks
retain source orientation, and numeric full closed traversals remain geometry.
The inline getter retains its separate full-traversal omission policy.
Explicit `Parameter` edits remain directed mathematical intervals. Direction
locking, B-rep edges and complete Copy memory remain
outstanding. See [scope](commands/subcurve.md) and
[provenance](standalone-subcurve-provenance.json).

The preceding 29-recipe command capture ran on private Xvfb with settings scheme
`VibocerosOracleSubcurveConfirmCommands20261007`. Twenty-eight commands succeed;
the standalone object-ID reference attempt remains a native cancellation.
[Raw records](../tools/rhino_oracle/observations/subcurve_numeric_followup.json)
retain 50 curves and 1,650 stations, complete originals, getter history and
command-end events. Real viewport and typed-coordinate confirmation picks
establish the step missing from earlier numeric diagnostics. Application tests
replay all 27 UV-command cases at `1e-6` for loci and directed endpoints, with
source purity and Undo/Redo. They cover magnitude/replacement inputs, forward and
backward orientation, curved sources, closed seams, nonuniform segment lengths,
open-end clamping, empty Enter and zero input. Drafting tests preserve units,
calculator expressions and point/angle routing. A self-crossing kernel regression
checks the original-domain endpoint without assuming which tied branch a
closest-point query would select. Direction locking and
B-rep edges remain unimplemented. See [scope](subcurve-length-confirmation.md)
and [provenance](subcurve-length-confirmation-provenance.json).

The preceding 20-recipe public SDK capture ran on private Xvfb with settings scheme
`VibocerosOracleSignedLengthFinal20261007`, returning 16 curve pieces and four
unavailable cases. The source NURBS definitions cover lines, curved polynomials,
stationary endpoints, piecewise spans, polylines, circles, ellipses and an arc.
[Raw records](../tools/rhino_oracle/observations/signed_length_subcurves.json)
retain definitions, 528 stations and measured source purity. Runtime Rust replay
compares every locus and directed endpoint at `1e-6`; the preceding standalone Python
geometry run additionally compares paired stations, with maximum discrepancy
`1.05031e-7`. Independent kernel tests check analytic forms, signed seam crossings,
one complete traversal and a quarter-unit interval after a `1e16` prefix.
Both UV command adapters accept explicit `SubCrvLength` options, with atomic
failure and source/history checks. These use numerical integration rather than
a continuous length certificate. Historical scripted/cursor getter diagnostics
remain recorded: those numeric inputs did not supply the required confirmation
pick and did not yield temporary curves. Those historical producers were
not retained; the fresh canonical SDK producer is hashed. See
[scope](signed-length-subcurves.md) and [provenance](signed-length-subcurves-provenance.json).

The preceding 14-recipe native capture ran on private Xvfb with settings scheme
`VibocerosOracleUVSubcurveClear20261007`. All inline `SubCrv` recipes succeeded,
creating 24 curves and two points. They cover forward/reversed inputs, closed
seam crossings, repeated ranges, mixed grouped whole objects and `SelNone`.
[Raw records](../tools/rhino_oracle/observations/uv_subcurve_input_command.json)
retain complete originals, 792 output curve stations, default temporary-input
attributes, layers, groups, getter events and independent Undo/Redo states.
Command tests compare every captured locus and directed endpoints at `1e-6`,
with source purity and complete metadata/history checks. Application tests cover
typed/mouse picking, invalid endpoints, cancellation, clearing and interruption.
The preceding standalone Python run reproduces all original source/range definitions
through the same command adapters. Ordinary normalized stations agree at
`1e-6`; one closed CreateUVCrv curve has identical ordered linear controls but
different knot spacing, retaining a maximum paired difference of `0.50625`.
Inline length input, direction locking, B-rep edges and that parameter-speed
policy remain incomplete; see [scope](uv-subcurve-input.md) and
[provenance](uv-subcurve-input-provenance.json).

The preceding 14-recipe native capture ran on private Xvfb with settings scheme
`VibocerosOracleUVFacesFinal20261006`. All 12 face-component-preselected
`ApplyCrv` and `CreateUVCrv` recipes succeeded; the two coordinate-input
attempts were rejected and remain in the
[raw records](../tools/rhino_oracle/observations/uv_face_reference_command.json).
Replay compares all 33 stations of every successful output curve at `1e-8`,
retaining the original six surface charts and face indices. Its reconstructed
B-rep contains independent faces and does not reproduce native shared-edge
topology. Separate tests cover every face of a connected kernel box, source
purity, Undo/Redo, ambiguous or invalid references, viewport hits, typed face
indices and component preselection. See [scope](uv-face-references.md) and
[provenance](uv-face-references-provenance.json).

The preceding standalone Python run reproduces nine original untrimmed CreateUVCrv
surface/input definitions. The Rust command replay additionally reconstructs
the two trimmed sources from their original UV curves. A create/apply roundtrip
recovers the original spatial curve within `1e-6`; application tests exercise
surface-first picking, optional point selection, preselection, cancellation and
World-XY output under a rotated construction plane.

The preceding 11-recipe native capture ran on private Xvfb with settings scheme
`VibocerosOracleCreateUVFinal20261006`, creating 17 UV curves and four points.
The [raw records](../tools/rhino_oracle/observations/create_uv_curves_command.json)
retain full inputs, trim curves, isocurve-length diagnostics, output stations,
properties, groups, selection and independent history states. The native coarse
rectangle size differs from accurate integration by about `3.00e-4` for the
radius-two cylinder. The off-surface curve's maximum paired discrepancy is about
`0.00686`; its endpoint also differs from the local closest-parameter projection.
These differences remain explicit in [provenance](create-uv-curves-provenance.json)
and [scope documentation](commands/create-uv-curves.md). Replay uses `1e-6` for
ordinary cases, `1e-3` for native primitive sizing differences and `0.01` for that
off-surface projection case. This does not establish full native sizing or
projection parity, or a continuous global nearest-locus certificate.

The preceding standalone Python command run reproduces all 13 original surface/input
fixtures, producing 17 curves and nine points. Undo and Redo replay outputs and
retain copied group definitions, including empty definitions for point-only
input. The two degenerate mapping rectangles produce no geometry or Undo entry;
`Undo` then reports nothing to undo and clears picking. Additional document tests
prove that retaining accepted group definitions never retains groups after a
failed transaction's rollback.

The preceding 13-recipe Rhino 8.32.26160.13001 capture ran on private Xvfb with settings
scheme `VibocerosOracleApplyUVPurity20261006`. Every recipe starts at idle in an
empty owned document with independently cleared Undo history. The
[raw records](../tools/rhino_oracle/observations/apply_uv_curves_command.json)
retain full source and target definitions before/after the command, command-end
events, 561 output curve stations, nine output points, properties, selection,
active group definitions and Undo/Redo states. Source and target geometry and
attributes remain unchanged. Rust replay compares the complete set of output
stations at `1e-6`, checks properties/group structure and exercises the actual
document adapter. Application tests verify source/target picking, preselection,
group filtering, aliases, cancellation and World-XY mapping under a rotated
construction plane. See [provenance](apply-uv-curves-provenance.json).

The preceding standalone Python pushup run certified images of all 13 original UV sources,
retaining their original domains. Eight diagnostic images have zero certified
error; the five curved primitive images have complete bounds at most `3.501e-16`,
below their requested limit `1e-6`. The Rust replay checks 1,677 returned-curve
stations against native surface-image witnesses and independently recomputes
every assembled result's continuous certificate.

The preceding 13-recipe Rhino 8.32.26160.13001 capture ran on private Xvfb with settings
scheme `VibocerosOraclePushup20261006`. Its
[raw records](../tools/rhino_oracle/observations/surface_pushup_certified.json)
retain 1,677 source/image stations, all source-purity agreements, five successful
native `Surface.Pushup` operations, and the sphere-seam parameter-speed
discrepancy. The other eight sources retain diagnostic reference curves. See
[provenance](certified-surface-pushup-provenance.json) for source hashes and
contract boundaries. That kernel API now backs the `ApplyCrv` / `ApplyCurves`
document workflow; `Pushup` is the SDK operation name.

The preceding standalone Python run certified all ten nonlinear singular-endpoint
source definitions without endpoint constraints, retaining each original
spatial domain. The polynomial recipes report bounds below `9e-16`; the
square-root boundary recipes refine to 103 controls with complete bounds about
`9.87e-7`, below their input limit `1e-6`.

The preceding ten-recipe Rhino 8.32.26160.13001 capture ran on private Xvfb with settings
scheme `VibocerosOracleInterpolatedPullbackFinal_20261006`. Its
[raw records](../tools/rhino_oracle/observations/surface_pullback_interpolation.json)
retain 1,290 source/image stations and ten source-purity agreements. Eight native
pullbacks succeed geometrically with different parameter speeds; both singular
cubic directions return no native pullback. The retained local before-change
audit failed on all ten recipes. See
[provenance](derivative-free-pullback-provenance.json) for source hashes and
contract boundaries.

The preceding standalone Python pullback run certified all 24 native sources without
endpoint constraints, returning two UV controls and each original spatial
parameter domain. All reported continuous bounds were below `1e-12`, with a
maximum of `8.86e-16`. The run used 16 iterations per source, retaining the
successful bound without recomputing its certificate.

The preceding 24-recipe Rhino capture ran on private Xvfb with settings
scheme `VibocerosOracleLinearPullbackFinal_20261006`. Its
[raw records](../tools/rhino_oracle/observations/surface_pullback_linear.json)
retain full source definitions, source purity, 129 stations per recipe, 20
successful native pullbacks, four native failures, and a parameter-speed
discrepancy for both directions of the swapped two-pole surface. The local
certificates and native geometric fitting have different contracts; see
[provenance](automatic-surface-pullback-provenance.json) and
[timing details](automatic-surface-pullback-performance.json).

The earlier eight-recipe capture also ran on private Xvfb with settings scheme
`VibocerosOraclePullbackEndpointsVerify_20261006`, using the eight
[`surface_pullback_endpoints` recipes](../tools/rhino_oracle/fixtures/surface_pullback_endpoints.json).
Its complete output was identical to the
[retained capture](../tools/rhino_oracle/observations/surface_pullback_endpoints.json),
including the planar parameter-speed differences and independent endpoint
witnesses. Both files had SHA-256
`71593c8aa90b81679ed1a196a5a25fd9738ecd9febdc71cd237934a3e0e436a0`.
The owned Rhino, Python worker, and Xvfb processes exited after the capture.

## What this establishes

The current checked cases cover geometry, document transactions, commands,
interchange, CPU viewport behavior, and Python orchestration. The historical
GPU evidence above separately covers production GPU rendering.
The complete workspace test process was observed to finish with exit status 0.
The nonlinear pullback replay adds 1,290 source and image stations at `1e-11`,
with independent recomputation of the final assembled-curve certificates.
Earlier automatic pullback tests check 3,096
paired spatial and surface-image stations at `1e-11`, independently recomputing
the local continuous bounds. Earlier fixed-endpoint tests retain another 1,032
stations. Geometry regressions preserve adjacent-float and subnormal domains,
singular endpoints, signed weight gauges, fixed constraints, and exact rational
linear crossings in both parameter directions.

The newest native capture covers 32 closed numeric recipes, 16 retained pending
point getters, and six cancellations. The preceding direction capture covers 18
recipes with four retained failures and four cancellations. The preceding capture covers a 37-step
preference sequence with raw cancelled queries and option edits. The preceding
capture covers 22 FromMidpoint recipes with three retained
cancellations. The preceding capture covers 15 MarkEnds recipes with two retained
cancellations. The preceding standalone capture covers 17 SubCrv recipes. The
numeric getter/command capture covers 29 recipes,
including one retained cancellation. The signed-length SDK capture covers 20
independent geometry queries. The earlier command capture
covers 14 successful endpoint-input SubCrv recipes.
The preceding face capture retains two rejected coordinate-input attempts.
Other recorded-output
replays are not fresh cross-engine measurements, and tests that merely execute
fixtures with finite output do not establish numerical agreement with Rhino.
Native `Surface.Pullback` and `Surface.Pushup` follow geometric loci and do not
promise these local fitters' normalized-parameter correspondence or fixed UV
endpoint constraints.
Passing these cases does not establish all-command coverage, arbitrary-geometry
epsilon agreement, general curved Booleans, general STEP B-rep interchange,
other graphics backends, or performance parity. Those remain subject to the
implementation boundaries in
the [architecture](architecture.md), [file-format](file-formats.md), and
[oracle timing](oracle.md#timing-interpretation) documentation.
