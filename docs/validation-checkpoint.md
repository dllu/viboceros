# Validation checkpoint

[Architecture and status](architecture.md) · [Rhino oracle](oracle.md)

This is a reproducible regression checkpoint, not a compatibility certificate.
The October 9, 2026 audit tested BlockEdit viewport input and double-click opening, BlockEdit controls and chronology, in-place BlockEdit, BlockResetScale, AddObjectsToBlock, exact affine inversion, ReplaceBlock input and replacement, unique/duplicate block definitions, BlockManager operations and UI, native block group replay, workflow metadata corrections,
block expansion, structural 3DM blocks, object snapping, Block/Insert commands, native instances, the catalog and bounded 3DM
traversal/reflections, STEP extrusion images, certified surface edits, curve Rebuild
previews and prepared surface proof contexts in the worktree based on
`a71ee68b` (earlier milestones used `4f513b99`, `6e60921e`, `ed496a1e`, `9e3cbf53`, `0cf4b460`, `7f9f07b6`, `dd130d9e`, `a4add84b`, `c0a405a3`, `368f1d1a` and `35f2b035`), with Rust 1.95.0.
Code and measurement hashes are retained in
[block editor input provenance](block-edit-input-provenance.json),
[block editor control provenance](block-edit-controls-provenance.json),
[in-place block editing provenance](block-edit-provenance.json),
[block scale-reset provenance](block-reset-scale-provenance.json),
[block addition provenance](block-add-provenance.json),
[replacement input provenance](block-replace-input-provenance.json),
[replacement provenance](block-replace-provenance.json),
[unique-definition provenance](block-unique-provenance.json),
[management provenance](block-manager-provenance.json),
[group provenance](block-group-provenance.json),
[workflow provenance](block-workflow-provenance.json),
[expansion provenance](block-explode-provenance.json),
[structural provenance](three-dm-structural-provenance.json),
[snap provenance](block-snap-provenance.json),
[command provenance](block-command-provenance.json),
[instance provenance](block-instance-provenance.json),
[block catalog provenance](block-definition-provenance.json),
[block audit provenance](three-dm-block-audit-provenance.json),
[extrusion provenance](step-extrusion-pcurve-provenance.json),
[STEP qualification provenance](step-spline-pcurve-provenance.json),
[surface-edit provenance](surface-edit-certificate-provenance.json),
[preview provenance](curve-rebuild-preview-provenance.json) and
[cross-target provenance](retrim-target-cache-provenance.json). Git metadata was
mounted read-only during the earlier captures. Git writes were enabled later on
October 8, allowing these previously validated changes to be committed.

## Commands and results

```sh
cargo test --workspace --release
python3 -m unittest discover -s tools/rhino_oracle -t .
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
git diff --check
```

All commands completed successfully. The complete release workspace run passed
5,556 Rust tests:

| Package | Passed | Ignored |
| --- | ---: | ---: |
| App | 1,086 | 17 |
| Command | 1,232 | 2 |
| Document | 255 | 5 |
| Drafting | 164 | 6 |
| Geometry | 2,103 | 8 |
| I/O | 212 | 0 |
| Oracle (unit and integration) | 504 | 0 |

The Python suite passed 1,052 tests. The 38 ordinarily ignored Rust tests were
not run in this checkpoint. The September 12 audit of `bd299074` separately
passed six opt-in GPU tests covering 182 renders on NVIDIA GB10 / Vulkan /
driver 610.43.02; that is historical evidence, not a new graphics check.
See [GPU tests](gpu-tests.md) for their pixel assertions and limits.

The newest [BlockEdit input](commands/block-edit.md) adds five application
state/getter tests, two viewport pointer/permission tests, one scene-visibility
test and two Python capture-adapter tests. External-source picking maintains a
separate highlight set and protects editable selection, supports selection
commands and region input, and cancels without changing the workspace. Base-point
input uses the shared snapped/filtered point pipeline and resolves CPlane
coordinates. Idle, unambiguous block double-clicks open the editor; active getters
retain ownership and title double-clicks retain viewport maximization. Source
eligibility is cached at opening; ordinary picking excludes released members.
A private-Xvfb native line-instance double-click capture verifies opening, one
exposed member, hidden root and exact model-ID restoration after discard. It is
one gesture witness, not broad native picking, ambiguity, snapping or history
qualification. Nested navigation, hidden/locked-member choices, open-editor
persistence, linked definitions and performance remain open.

The preceding [BlockEdit controls](commands/block-edit.md) add six document tests,
one command test, one egui pointer test, one replay test and six Python tests.
All sixteen native workflows match 82 model states, with maximum sampled error
`8.9e-16`, for source
copying, release on save, discard, base points, unchanged saves, SDK/command Move,
groups, nested references and a reflected circle. Root placements and original
metadata survive. Copied nested references clear geometry text while their
sources retain it. Member serialization follows temporary object chronology.
Save scans members once; the chooser renders visible rows without repeated
object-table searches. No speedup is inferred from these implementation changes.
Native controls run from idle through public WPF/Win32 inspection, actual pointer
clicks and a getter readiness/completion handshake. Nested navigation, external
source/base-point viewport picking, hidden/locked-member warning choices, linked
definitions, saved active editors and performance qualification remain open.

The preceding [BlockEdit](commands/block-edit.md) adds seven document tests, one
command test, two GUI tests, one native replay test and two Python validation
tests. Eight private-Xvfb workflows match all 41 recorded model states for save,
discard, translated/reflected placements, nonuniform rejection, nested references,
member/root text and group handling. Save refreshes all original roots as one
model history entry; discard restores the original document and redo branch.
Local tests cover created members/layers, protected deletion and group selection,
failed cyclic saves and file-open/export guards. Captures use point members and
nested references through the public native command/SDK lifecycle; temporary
object, selection/history and panel parity remain unverified. Add/Remove Object,
Set Base Point, nested navigation, double-click opening, linked definitions and
GPU/performance qualification remain open.

The preceding [BlockResetScale](commands/block-reset-scale.md) adds six document
tests, two command tests, two GUI tests, one native replay test and two Python
validation tests. All 77 private-Xvfb workflows match 388 whole-model states
for One/Automatic, reflections, tilted shear, metadata/groups, preselection,
cancelled-choice memory and numeric pair boundaries. The largest sampled
difference is `7.11e-15`. Near-equality uses an empirically calibrated absolute
and relative band; wider numeric qualification remains open. Local tests cover
subnormal ratios, staged geometry failure, protected/missing/mixed batches and
history. Shared typed cancellation now follows command-owned handling, retaining
Smooth's established workflow. Native mouse/group picking, selection/history
parity, linked definitions, GPU rendering and performance remain unverified.

The preceding [AddObjectsToBlock](commands/add-objects-to-block.md) adds four kernel
tests, five document tests, two command tests, three GUI tests, one native replay
test and four Python validation tests. Eight private-Xvfb native workflows match
all 34 states for source consumption, inverse placement, nested references,
root/member metadata, model/prototype groups and circle/arc fitting. Addition
clears all target prototype memberships while retaining model group records.
The exact affine inverse handles cancellation, extreme scales and overflowing
intermediate translation terms without accepting exact singularity. A scoped
OpenNURBS Rust adaptation reproduces circular fitting under nonuniform placement;
general affine editing keeps its exact-conic behavior. Local history tests verify
catalog refresh, group/object restoration and atomic graph rejection. Linked
definitions, native mouse/group picking and selection/history parity, layer-level
protection, general curved B-rep comparison, GPU rendering and performance remain
open.

The preceding [ReplaceBlock input](commands/replace-block.md) adds six GUI tests,
one native replay test and four Python validation tests. Three private-Xvfb native
workflows select the target through Rhino's object getter and match all 23 states
for selected-only and All replacement, protected peers, and target state
transitions. Hidden and locked modes are mutually exclusive in that probe, and
normal restores target-getter eligibility. Local egui pointer events constrain
chooser acceptance/cancellation, removed rows and changed source permissions.
Target picking keeps source selection and target groups intact. The explicit
name getter accepts scope words, command names and equals signs as names.
Native chooser interaction, option memory, history parity, linked definitions,
GPU rendering and performance remain unverified.

The preceding [ReplaceBlock workflow](commands/replace-block.md) adds two document
tests, two command tests, one GUI test, one replay test and two Python validation
tests. Six private-Xvfb native workflows match all 37 states for selected-only
and All scope, protected peers, reflected nonuniform placements, root metadata
and groups, and unchanged nested definition references. Replacement retains root
IDs and placements but clears geometry-attached root text. Local tests constrain
atomic preparation, protected explicit selection and Undo/Redo. Instance picking
and the definition chooser are supplied by the newer input milestone above;
layer-level protection, native option memory, linked definitions and performance
remain open.

The preceding [unique-definition workflow](commands/create-unique-block.md) adds
three document tests, two command tests, two GUI tests, one replay test and two
Python validation tests. Five private-Xvfb native workflows match all 30 states
for placement, root identity/metadata/groups, shared child definitions, subsequent
child redefinition and unused duplication. Copied prototype groups are cleared.
Immutable member geometry stays shared until later edits; rebinding and catalog
creation form one Undo step. Native name defaults, protected/group selection,
linked definitions, panel duplication interaction and performance remain open.

The preceding [BlockManager](commands/block-manager.md) adds three document tests,
three GUI/action/rendering tests, one interface parser test, one replay test and
two Python validation tests. Four private-Xvfb Rhino workflows match all 33
states for nested use counts, rename, allowed deletion and the declared manager
guard. UseCount measures placed occurrences; active prototype reference counts
remain separate. SDK Delete(True) can remove nested references, so the reference
adapter explicitly applies the published manager restriction. Local history
tests preserve IDs, geometry storage, groups, hidden/locked states and redo.
The pane uses natural name order and protects conflicting rename drafts.
Native panel input, protected-root deletion, GPU rendering, linked definitions,
duplicate/export actions and block editing remain unverified or unfinished.

The preceding [block group follow-up](block-groups.md) adds three document tests,
one oracle unit test, one replay integration test and four Python tests. All 16
native command workflows, including 82 whole-model states, match their geometry,
catalogs, ordered group memberships and group tables. Block clones top memberships
into fresh prototype groups; Explode copies them again, while ExplodeBlock reuses
leaf groups and discards nested-container/root memberships. GroupOutput creates
one extra group per root in a batch. SDK construction strips group indices; its
14-case replay retains eleven diagnostics and three matches, without execution
failures. Local tests independently constrain group allocation, rollback and
Undo/Redo. Native option memory, protected-root selection and history remain open.

The preceding [block workflow oracle](block-workflow-oracle.md) adds three oracle unit
tests, two native replay integration tests, two document tests and five Python
validation tests. Fresh Rhino 8.32.26160.13001 captures ran on private Xvfb:
20 command workflows, 20 SDK workflows and four root-text follow-ups. All 22
command workflows agree on definition graphs, transforms, geometry and supported
metadata at absolute `1e-9` plus relative `1e-12`; the main 20-case batch records
85 states and 3,477 geometric stations. The largest command numeric difference
is `1.43e-14`. Native evidence corrects raw ByParent preservation and geometry-text
loss when a conic converts to NURBS. SDK color/text/representation differences
remain explicit diagnostics. These are sampled checks, not continuous geometry
certificates or performance measurements. Native grouping, selection history,
option memory and protected-root behavior remain unverified.

The preceding [block expansion](commands/explode-blocks.md) adds five document tests,
three command tests and two GUI tests. One-level Explode retains nested instance
relationships; recursive ExplodeBlock returns geometric members. Metadata and
the initial ByParent appearance policy are preserved at that checkpoint;
prototype groups are copied per placement
scope, and root groups remain attached to outputs. Preparation/commit checks
source/layer/tolerance freshness and output budgets. Tests cover geometry storage
on Undo, mixed ordinary/block history, protected member states, independent groups,
AllBlocks, grouping, cancellation and stale-plan failure. Existing ordinary
Explode cases passed. Native selection/option-memory/protected-root and multi-root
grouping parity remain unverified; no fresh Rhino capture was made.

The preceding [structural 3DM path](three-dm-structural-blocks.md) adds six I/O tests
and two command tests. Native definitions, member UUID lists and instance records
retain supported embedded graphs across export and editable import. The legacy
flattened reader independently agrees on placed point/line/cloud/B-rep geometry;
unit conversion preserves each linear map and scales every translation once.
Tests cover hidden members, hidden/locked roots, malformed graph rollback,
existing-destination preservation, shared redefinition and isolated import name
conflicts. Structural reading rejects linked definitions and unsupported members;
prototype object-lock modes reject export. Broader definition/model metadata and
fresh Rhino cross-reader/performance evidence remain unverified.

The preceding [block object snapping](block-snapping.md) adds five drafting tests
and two CPU viewport tests. Existing feature and intersection queries now use
immutable member records with independent cache IDs and root result ownership.
Point mode also includes root/nested insertion points, including empty nested
references. Tests cover all existing modes, visibility/locking, repeated
placements, lazy-cache reuse, definition edit/Undo/removal and mesh intersections
inside one instance. Four-view cursor checks confirm Point/End ownership and
mesh-wire admission. Ordinary documents avoid an expanded-source vector;
suspended queries remain cold. The full legacy snap suite passed alongside these
checks. No fresh native block-snap capture, GPU run or timing comparison was made.

The preceding [Block/Insert commands](commands/blocks.md) add five document creation
tests, four command tests and five GUI getter tests. They check base normalization,
source replacement, nested references, redefinition, group/raw-text retention,
immutable geometry sharing, cancellation and atomic failures with redo. Extreme
insertion scale uses the existing OpenNURBS cardinal-angle cleanup; a 90-degree
rotation at scale `1e308` retains zero off-axis coordinates. Quoted names containing
`=` work in interactive starters and complete scripts. The command help list was
updated for both registrations. The member-layer lock policy now follows the
published Rhino rule; member object-mode locking remains a local display rule.
These are documented-rule/local regression checks, not fresh native command
captures. External/linked insertion, block editing tools, snapping, subobject
editing, structural 3DM interchange and broader selection/option-memory parity
remained unfinished at that preceding checkpoint.

The preceding [instance objects](block-instances.md) retain definition relationships
through root selection, affine editing, transformed copies, history and unit
rescaling. Nine document tests check stale/foreign values, atomic live-instance
refresh, group/metadata retention and original geometry storage on replay. A
reflected box retains physical volume and continuously certified face boundaries
after unit rescaling. Two command tests check Move and a 3DM export rejection
that preserves the existing destination. Three CPU viewport tests check root click/
window selection, cache invalidation and exact scene-buffer equality across
four views, three display modes and selected/unselected states. They do not
execute the GPU backend. At that preceding checkpoint, Block/Insert commands, block snapping, subobject
editing and structural 3DM round trips remained unfinished; no fresh Rhino block-command
capture or timing comparison was made.

The preceding [native block catalog](block-definitions.md) adds 12 regressions for
shared immutable definitions, noncommuting nested placement, paths/raw metadata,
atomic graph validation, Undo/Redo, live-layer constraints and resource bounds.
Unit rescaling stages definition geometry/reference translations and ordinary
objects before mutation; failures in either table preserve both tables and redo.
The final document-only release run after lint fixes passed 199 tests with five
ignored. At that preceding checkpoint, instance objects, interactive Block/Insert commands,
parent-attribute inheritance and structural 3DM round trips remained unfinished. No fresh Rhino
block editing or timing evidence was captured.

The preceding block traversal adds a one-million-visit ceiling per top-level object,
bounding empty-definition branching graphs independently of leaf geometry count.
An acyclic 20-level fixture proves this failure path. Additional placements check
reflected B-rep boundary correspondence/volume, stored cloud normals and invalid
projective/singular maps. Command replay checks each failure preserves the document
and redo history. The normal expectation follows the public OpenNURBS transform
implementation, retaining its stored channel. Earlier source/fixture hashes are
preserved. Fresh Rhino cross-reader, definition editing and performance evidence
remain unverified. See [scope](three-dm-blocks.md).

The preceding 3DM reader expands supported nested/repeated block references as
independent placed geometry, suppressing unplaced definition prototypes and
reporting top-level expansion counts. The bridge resolves members and inherited
attributes; the Rust kernel applies checked affine placement before unit scaling.
Three I/O regressions verify noncommuting transforms, source/placed coordinates,
colors/groups, hidden locks, channels, box topology/volume and malformed graphs.
Two command regressions cover import/export, independent Undo/Redo and atomic
failure preserving redo. Original public OpenNURBS fixtures include both valid
models and intentionally missing/cyclic references. Native block-definition
editing/structural round trips, external linked-file resolution, fresh Rhino
cross-reader and performance evidence remain unfinished/unverified.
See [scope](three-dm-blocks.md).

The preceding linear-extrusion adapter constructs certified spatial edge images
for diagonal and curved rational p-curves on parameter-preserving directrices.
Face and edge paths share tensor construction. Direct spline/parameter-curve
tests cover reversed/signed UV gauges, crossed directrix knots, constant axial
coordinates and invalid charts. Serialized polynomial/rational extrusion triangles
retain editable surfaces, original p-curve domains and continuously certified
oriented trim/edge correspondence. The existing globally affine line route stays
first, preserving its unbounded parameter map; conic/angular directrices retain
their preceding isocurve policy. No fresh Rhino cross-reader or timing is claimed.
See [scope](step-extrusion-pcurves.md).

The preceding STEP qualification layer covers every p-curve edge on a B-spline/NURBS
basis, including affine, straight, extracted isocurve and rational reparameterized
paths. Proposal construction remains separate in its existing adapters; the final
edge must receive a continuous original-UV/surface proof at actual import tolerance.
A rational straight-path regression accepts model tolerance and rejects the same
rounded edge at a stricter tolerance. The preceding curved-only sources are retained
with their original hashes, and analytic/explicit 3D-edge adapters retain their
existing contracts. See [scope](step-pcurve-certificates.md).

The preceding native STEP curved spline p-curve composition independently certifies
its final joined spatial edge against the original stored UV curve and spline
surface at absolute import tolerance. Rounded splits/snapped crossing endpoints
remain proposals. Full-order surface-knot components are selected by exact
control/knot slices when the sign-coherent UV hull fits one component; paths
on a break keep the following-side convention, and discontinuity crossings fail.
Tolerance propagates through nested curve leaders and sweep directrices. Two
regressions reject a between-station excursion and check exact two-axis component
slices. The retained crossing matrix and serialized oriented trim/edge checks pass.
This proof applies to the curved spline p-curve path; other native adapters keep
their preceding contracts. No fresh Rhino cross-reader or timing is claimed.
See [scope](step-pcurve-certificates.md).

The preceding single-face control-net edit replaces sampled-only boundary images
with continuously qualified fitted images at one quarter of absolute model tolerance.
The existing proposal fitter retains its 4,096-control ceiling; dense edited
trims receive an explicit 16-million-unit continuous proof budget per image/use.
Shared edge reuse certifies
every UV use, and singular edge removal requires a zero-error constant-image
proof. Five geometry regressions cover diagonal/reversed boundaries, circular holes,
mixed-weight rejection, shared sphere seams/poles and an excursion hidden at old
fit stations. A certificate regression checks explicit budget exhaustion and unchanged tolerance. A document regression
checks certified Smooth hole images with metadata and independent history.
Retained native Smooth comparisons remain sampled nearest-locus witnesses;
the local continuous proof does not establish identical native edge controls or
parameter speed. General singular edits, global embedding and performance parity
remain unverified. See [contract](surface-edit-certificates.md).

The preceding preview audit shares the staged-document selector with production
drawing and checks replacement/copy/failure/cancel routing across display modes.
Compact curve/surface option buttons use typed handlers, return command-field
focus and disable Accept after preparation failure. Egui click tests check
toggles, value prompts, exact prepared-output acceptance, U/V independence and
stale-source rejection before buttons render. Expanded tests cover all guarded
source categories and retain previews after equal-geometry no-op assignments.
Mixed-selection preparation and current-layer identity/history policies remain
atomic. Production Xvfb inspection was attempted; X11 launch failed because the
sandbox prevents listening-socket binding. No completed production screenshot
or new native comparison is claimed. See [guide](commands/rebuild-curves.md).

The preceding curve Rebuild workflow prepares readonly typed previews after
preselection or picking and accepts the exact prepared geometry in one history
step. Deletion/layer changes reuse output storage; geometric options rebuild it.
Failed preparation blocks acceptance, invalid syntax retains the preceding
preview, and cancellation retains document history/redo. Source guards cover
geometry, attributes, group/root text, selection, tolerance and current layer.
Three command tests and five app tests cover preparation, partial options,
closed curves, output reuse, background refresh, stale inputs, failure recovery,
inline/postselected routes and independent Undo/Redo. The existing curve kernel
and output policy remain in use. Curve option-memory/native UI parity and new
production pixel comparisons remain unverified. See [guide](commands/rebuild-curves.md).

The preceding fixed-curve search reuses bounded coefficient state across independent
targets. Original, translated and failed-translation refinement frames stay separate;
each retains at most 64 spans. Exact net state owns coefficients while every query
supplies the unchanged curve and validates its denominator anew. Two new regressions
check transferred sided jets after eviction/invalid stations and alternating target
translation fallback. Sphere/swapped/cylinder medians are 0.8524/1.2066/0.5171 seconds,
with all source and complete output definitions identical to preceding profiles.
These small timing changes do not establish a general speedup. The preceding
projection source hashes remain verified in `retrim-projection-source/`.
See [measurements](retrim-target-cache-provenance.json). No new native timing is claimed.

The preceding projection preparation retains three repetitions of the same cases.
Swapped-cap median retrim time falls from 1.3417 to 1.2219 seconds; sphere/cylinder
medians of 0.8644/0.5143 seconds remain near their preceding values. All source and
complete output definitions remain identical. Fixed isocurve searches reuse seed
images and translated controls while preserving every start, refinement limit,
original-coordinate comparison and failed-translation fallback. Opt-in evaluation
queries retain at most 64 coefficient spans. Chart projection reuse keys exact UV
bits, including periodic lifts, and caps entries at 131,072. Tests compare prepared
searches and public jets under extreme inputs, eviction, invalid queries and repeated
chart hits. The prepared-context sources are archived with verified hashes.
See [measurements](retrim-projection-provenance.json). No new native timing or
global nearest-point certification is claimed; performance parity remains unresolved.

The preceding prepared surface context retains three separate repetitions for the
same sphere cap, swapped cap and cylinder band. Independent image proposals reuse
exact surface extraction, interval patches and coefficient tables, with a fresh
2,000,000-unit rational work budget for each candidate. Cache hits reduce charged
work without changing the proof arithmetic, tolerance or subdivision limits.
Sphere-cap median retrim time falls from 1.3905 to 0.8764 seconds; the other cases
remain near their preceding values. All source and complete recorded output
definitions remain identical. A regression compares repeated prepared and
standalone proofs across weight gauges, domains, offsets and invalid limits,
including success after rejection. General resource-admission equivalence is
not established. The preceding cache sources and hashes are preserved in
`retrim-cache-source/`. See [measurements](retrim-prepared-provenance.json).
No new native timing is claimed; substantial performance work remains.

The preceding preparation cache retains three separate repetitions for the sphere
cap, swapped cap and cylinder band. Sphere-cap median retrim time falls from
1.6077 to 1.3905 seconds; the other two cases remain near their previous values.
All three retain their preceding source geometry and complete recorded output
definitions. Fixed target isocurves are extracted once per projection fit.
Certified interval patch controls are converted once per immutable tensor patch,
including remembered inconclusive conversions that continue through exact fallback.
The same proof arithmetic, work charges and tolerances remain. Earlier proof
sources are archived from `73553044` so historical hashes remain verifiable after
new source edits. Full release workspace tests, Python tests, Clippy, formatting
and diff checks pass. See [measurements](retrim-cache-provenance.json) and
[performance guide](retrim-performance.md). No new native timing is claimed;
the preceding Rhino timing gap and broader performance limits remain unresolved.

The preceding trim-transfer profile retains three baseline and three optimized
radius-two sphere-cap runs on this DGX Spark. Median release retrim cost falls
from 64.7758 to 1.6077 seconds, about 40× for this case. Each emitted geometry
definition has the same canonical fingerprint across the runs, including its
73 edges. Temporary instrumentation located most of the original cost in
continuous image proofs and was removed before the final verification.
Outward finite intervals now enclose Bernstein composition and homogeneous
differences for nonlinear UV against cubic spatial spans. Rational conversions
are checked exactly, reported Euclidean bounds are checked with rational squared
arithmetic, and ambiguous, overflow/subnormal or zero-limit cases use exact
fallback. At knot crossings, restricted box enclosures can prove correspondence
or prove only that the box method requires subdivision; that latter outcome is
never geometric rejection. Existing rational extraction and tolerance remain.
Floating attempts have 65,536 work units and eight hull levels; degree and
rational work/bit/depth limits remain in place. Coefficient tables are cached
within each prepared surface. Six independent regressions compare arithmetic,
curved spans and crossing boxes to exact rational results, exercise extreme
values, and reject an excursion hidden at all four cubic sampling nodes.

A fresh [private-Xvfb SDK capture](../tools/rhino_oracle/observations/surface_retrim_profile.json)
retains three repetitions each for the cap, swapped cap and cylinder band, using
the serialized local source surfaces and trim bounds. Every result is valid;
sources remain unchanged. Native target controls match at `1e-6`, knots at
`1e-12`. Source setup, serialization and disposal are outside reported method
timings. Rhino's cap median is 0.02768 seconds versus local 1.6077: substantial
performance work remains. Different validation contracts, Python/RhinoCommon
and Wine/FEX overhead, three samples and one host preclude a general kernel
speedup or parity claim. The full app, command and geometry runs retain the
preceding native boundary, source-purity and history checks. See
[performance guide](retrim-performance.md) and [raw profiles/provenance](retrim-performance-provenance.json).

The preceding singular trim Rebuild capture ran twelve recipes on a fresh private
Xvfb scheme. [Complete records](../tools/rhino_oracle/observations/surface_rebuild_singular_trim.json)
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
swapped cap. [Sixty-six independent closest queries](../tools/rhino_oracle/observations/surface_rebuild_cap_projection.json)
show about `0.000474` variation in closest U, while native trim transfer keeps U
within `3e-8`. A full constant-U contour on a closed V chart now transfers to the
target isocurve at its projected shared endpoint. The swapped cap retains two edges
like Rhino; normal/reversed caps have 73 local edges versus two native, the sphere
hole 84 versus two, and the cone hole 83 versus three. Extra segments are individually
certified and remain an explicit representation difference. The initial ten-result
artifact is incomplete evidence for its twelve-case request. Both ordinary engine
runners now require the exact requested recipe ID set, rejecting missing or extra
results while allowing reordering. See [guide](commands/rebuild-surfaces.md) and
[provenance](singular-trim-rebuild-provenance.json). Interior singularities,
multi-chart winding, general solid embedding, broader isocurve-transfer families,
global nearest-point guarantees and performance parity remain unverified.

The preceding trimmed closed-surface Rebuild captures ran eight cylinder patch/hole
recipes and two seam-crossing hole recipes on fresh private Xvfb schemes.
[Patch/hole records](../tools/rhino_oracle/observations/surface_rebuild_seam_trim.json)
and [crossing-hole records](../tools/rhino_oracle/observations/surface_rebuild_crossing_hole.json)
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
See [guide](commands/rebuild-surfaces.md) and [provenance](seam-trim-rebuild-provenance.json).
Generic singular trims, multi-chart winding, global closest-point guarantees,
continuous projection-error bounds and performance parity remain unverified.

The preceding closed surface Rebuild captures ran sixteen default and twelve
varied-degree/count recipes on fresh private Xvfb schemes. [Default records](../tools/rhino_oracle/observations/surface_rebuild_closed.json)
and [degree/count records](../tools/rhino_oracle/observations/surface_rebuild_closed_degrees.json)
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
See [guide](commands/rebuild-surfaces.md) and [provenance](closed-surface-rebuild-provenance.json).
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
outcomes. A [four-command Xvfb capture](../tools/rhino_oracle/observations/surface_rebuild_natural.json)
confirms full target boundaries and retained source domains on default 10x10 warped
and curved natural faces. Exact target isocurves preserve reordered edges, reversed
uses and vertex indices while avoiding inverse fitting; straight UV contour images
use canonical parameter speed for direct knot-crossing composition. The first eager
previews exposed work limits/slow inverse fits on those boundaries.
An [eight-stage production UI inspection](../tools/rhino_oracle/observations/rebuild_preview_ui.json)
completes replacement/copy previews, an invalid edit, acceptance, Undo/Redo and
cancellation on private Xvfb. Screenshots show two drawn surfaces in a copy preview
while the document layer pane still reports one object. Early UI observers misread
active/invalid command text; the final command-field observer completed successfully.
See [guide](commands/rebuild-surfaces.md) and [provenance](rebuild-preview-provenance.json).
Curve previews, seam/singular retrimming, split natural-side specialization, native
pixel appearance, general closest-point guarantees and performance parity remain
unfinished or unverified.

The preceding surface Rebuild retrim captures ran fourteen owned native commands
and two public trim-transfer queries on private Xvfb. [Initial records](../tools/rhino_oracle/observations/surface_rebuild_retrim.json)
and [follow-up records](../tools/rhino_oracle/observations/surface_rebuild_retrim_followup.json)
retain complete source/output topology, controls, 33 edge stations, 81 surface
stations and independent history for planar holes, nonuniform parameterization,
warped cuts, curved/rational sources and natural boundaries. Every native
ReTrim=Yes outcome equals public Brep.CreateTrimmedSurface and retains source UV
domains; UV copying disagrees materially on the nonuniform source. Local command
replay checks controls at `1e-6`, native edge distance witnesses at `2e-6`, source
purity and history. A [two-query API capture](../tools/rhino_oracle/observations/brep_retrim_geometry.json)
projects a paraboloid annulus onto parallel planes. API replay checks loops,
vertices and edge witnesses at `2e-6`. App and analytic regressions check holes,
area, orientation, readonly edits and trim/edge certificates. A default 10x10
circular-hole regression initially exhausted the general knot-crossing proof;
an exact affine control-net reference certificate now proves that case while
retaining rational image controls. Incorrect translated images and mixed-sign
UV proposals are rejected. See [guide](commands/rebuild-surfaces.md),
[certificate](surface-curve-certificates.md) and [provenance](surface-rebuild-retrim-provenance.json).
Projection searches and fits have sampled accuracy checks; new spatial edges
are continuously certified against their new UV trims. Global closest-point
projection, seam/singular trims, native previews and performance parity remain
unfinished or unverified.

The preceding surface Rebuild option captures ran a 25-step preference sequence and
nine-step ordered degree/count follow-up on fresh private Xvfb schemes.
[Primary prompts](../tools/rhino_oracle/observations/surface_rebuild_options.json)
and [ordered follow-up](../tools/rhino_oracle/observations/surface_rebuild_option_followup.json)
show immediate persistence of every valid option edit through cancellation and
Undo/Redo. Scripted degree growth raises the count; a count below the current
degree minimum is rejected. App replay compares all 34 initial/final states,
source purity, cancellations, accepted control nets and independent history.
Numeric value questions, inline versus picked invocation, unchanged invalid
edits and new-registry isolation have separate regressions. Shared command-owned
parsing preserves token order and keeps preferences outside document history.
See [option provenance](surface-rebuild-options-provenance.json). Native dialog
coupling, restart persistence, curve/surface preference interaction, previews and
trim projection remain unfinished or unverified.

The preceding surface Rebuild captures ran twelve owned native commands and twelve
public SDK queries on private Xvfb. [Command records](../tools/rhino_oracle/observations/surface_rebuild.json)
retain complete surfaces, 81 stations, attributes/groups, source identity and
independent history for polynomial/rational inputs and all copy/replacement and
input/current-layer combinations. Native command geometry exactly equals public
SDK results. Local replay uses `1e-6` for controls and exact knots/weights.
The [standalone API audit](../tools/rhino_oracle/observations/surface_rebuild_local_audit.json)
matches all twelve asymmetric/reduced target structures, with maximum coordinate
error about `2.2e-7` and zero local failures. Its initial audit exposed two rational
arc-length endpoint overshoots by one binary64 step; endpoint parameters now use
exact domain boundaries. Shared rebuilding retains the preceding TweenSurfaces
regressions. App checks command-first picks, inline options, replacement and one
Undo. Command regressions reject unsupported retrimming and invalid options
without partial edits, and check explicit `ReTrim=No`. See
[guide](commands/rebuild-surfaces.md) and [provenance](surface-rebuild-provenance.json).
General retrimming, periodic/singular inputs, mixed curve/surface batches, native
option persistence and performance parity remain unfinished or unverified.

The preceding control-matching captures ran 38 successful owned commands on private
Xvfb with one, two and three outputs. [Initial records](../tools/rhino_oracle/observations/tween_surfaces_control_initial.json)
and [follow-up records](../tools/rhino_oracle/observations/tween_surfaces_control_followup.json)
retain unequal polynomial/rational degrees and counts, single-axis changes,
swapped order, cubic/curved nets and compatible nets with differing knots.
Kernel and command replay compare controls/positions at `1e-6`, knots/weights
at `1e-12`, source purity, full attributes/groups and independent Undo/Redo.
Public Surface.Rebuild records check preparation independently. Unequal nets
use midpoint-isocurve arc-length stations and uniform Greville tensor solves,
rebuilding before every output; compatible nets retain first-source knots.
An analytic plane regression and readonly three-output app replay add independent
checks. Solves normalize coordinates and check pivots, finite values and residuals.
Separate 26-case diagnostics retain rejected blend/row preparation hypotheses
and native option-change object effects. See [control provenance](tween-control-provenance.json).
Periodic/general singular inputs, extreme rational gauges, continuous source-locus
preservation and performance parity remain unverified.

The preceding repeated-corner capture ran 14 successful owned commands and 28 real
mouse clicks on private Xvfb under `VibocerosOracleTweenCornerSequencesVerified20261008`.
[Complete records](../tools/rhino_oracle/observations/tween_surfaces_corner_sequences.json)
retain each public viewport calibration, actual click, full source/output surfaces,
properties and independent history. Native swap toggles the swap bit without
transposing reversal bits; this corrects the earlier local composition after a
reversal. App replay finds current controls at each original world corner and
compares 81 normalized witnesses per source/output at `1e-7`, source purity and
history for every sequence, including inactive corners, orientation restoration
and three-click combinations. Queued corner edits now reject stale source geometry,
attributes/groups or document settings. A failed later preparation discards the
old scene, blocks acceptance and permits recovery. Initial stalled driver traces
are retained separately; the corrected driver waits for mouse-event acknowledgements
and completed without manual recovery. See [workflow](commands/tween-surfaces.md)
and [provenance](tween-sequences-provenance.json). General periodic/singular source
corners, coincident controls, native preview pixels, restart persistence,
option-change object effects and relative performance remain unverified or unresolved.

The preceding TweenSurfaces corner capture ran nine successful owned outcomes on
private Xvfb under `VibocerosOracleTweenCornerVerifiedFinal20261008`: one baseline
and eight real mouse clicks. [Complete records](../tools/rhino_oracle/observations/tween_surfaces_corners.json)
retain public viewport matrices, WorldToClient targets, actual integer mouse
pixels, full surfaces and independent history. End origin swaps U/V, end U and
V corners reverse their axes, and the opposite/start corners have no action.
App replay checks every source/output with 81 normalized witnesses at `1e-7`
and history. Three controls now draw and hit independently of snaps in every
viewport, following current end parameterization. Positions are cached with the
readonly scene; source selection/value questions hide them, and failed previews
retain them for recovery. Independent tests cover axis composition, source purity,
pixel hit/miss/clipping and Wireframe/Shaded/Ghosted modes. A private-Xvfb
inspection checks U, moved-origin swap and V clicks, Ghosted display, readonly
preview, acceptance and Undo/Redo. See [workflow and image](commands/tween-surfaces.md)
and [provenance](tween-corners-provenance.json). Initial recovered runs remain
separate diagnostics; the final run completed without manual recovery. Repeated
native click sequences, periodic/singular/overlapping controls, native colors/
labels/pixels and performance parity remain unverified.

The preceding TweenSurfaces option capture runs a 29-step owned private-Xvfb sequence
under `VibocerosOracleTweenOptionsVerified20261008`. [Complete records](../tools/rhino_oracle/observations/tween_surfaces_options.json)
retain native prompt defaults before/after edits, accepted geometry, cancellations
and independent history. App replay checks each subsequent invocation against
those defaults: count, method and sample edits commit only on success, while
OutputLayer saves in the options phase even on cancellation. Source-selection
cancellation accepts no inline layer preset; invalid edits preserve values.
An [eight-step follow-up](../tools/rhino_oracle/observations/tween_surfaces_sample_memory.json)
under `VibocerosOracleTweenSampleMemory20261008` confirms an edited sample count
survives accepted Refit and is revealed when returning to SamplePoints. Preferences
remain outside document history; command tests check independent registries,
reused documents and omission of source IDs/directions from memory. A cancelled
native Refit edit leaves two source-shaped copies even though its method change
is discarded; this remains an explicit geometry discrepancy. Restart persistence,
native corner-click directions, general geometry and performance parity remain
unverified. See [workflow](commands/tween-surfaces.md) and [provenance](tween-options-provenance.json).

The preceding TweenSurfaces interaction capture ran nine owned private-Xvfb recipes
under `VibocerosOracleTweenInteraction20261008`: three acceptances and six
cancellations with zero, one or two preselected sources. [Complete records](../tools/rhino_oracle/observations/tween_surfaces_interaction.json)
retain native prompts, geometry and selection/history. App replay checks all
nine outcomes with 81 normalized witnesses per source/output at `1e-7` and exact
restoration. Ordered picks now retain source order and allow repeated sources;
preselected sources remain selected across acceptance, cancellation and history.
Readonly preview scenes cache prepared geometry and do not edit the live document.
Typed options/directions update the scene, invalid edits retain it, and acceptance
reuses it in one transaction. Independent tests cover reversed picks, unchanged
cache reuse, invalid/pending values, cancellation, stale source/settings checks
and preserving an unrelated active transaction on acceptance failure. A private
Xvfb inspection of the production wgpu/egui path checks ordered viewport picks,
three-result preview, FlipEndU, Ghosted mode, invalid count retention, acceptance,
Undo/Redo and cancellation; saved images show preview/live layer counts of two
sources before acceptance and five objects after Redo. See [workflow and images](commands/tween-surfaces.md)
and [provenance](tween-interaction-provenance.json). Native corner-click directions,
option memory, option-change object side effects, native pixels and performance
parity remain unfinished or unverified.

The preceding Refit capture ran 24 owned public commands on private Xvfb under
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

The newest unique-definition capture covers five workflows and 30 states. The
preceding management capture covers four workflows and 33 states. The preceding
group capture covers 30 workflows, including 16 matching command
cases and 14 SDK-construction cases with eleven diagnostics. The preceding block
capture covers 44 API/command workflows, including 22 matching
command cases and retained SDK diagnostics. The earlier curve capture covers
32 closed numeric recipes, 16 retained pending
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
