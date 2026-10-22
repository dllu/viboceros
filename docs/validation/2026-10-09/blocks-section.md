# Section, blocks and 3DM history

[Historical checkpoint](../../validation-checkpoint.md)

[Section](../../commands/section.md) adds three kernel mesh tests, two
command tests, two application getter tests, one native integration test and two
Python validation tests. Plane cuts use the CPlane trace, finite-span clipping,
current/input properties, per-plane grouping and repeated point input. Surface
and B-rep paths reuse the existing validated intersection kernel. Mesh edge
intersections use exact dyadic arithmetic and shared endpoint keys, retain face
winding and trace branches without joining through nonmanifold junctions.
Thirteen of fourteen native recipes match strict sampled geometry, domains,
metadata and groups. One coplanar mesh recipe retains a different seam/domain and
two extra collinear vertices in Rhino; raw comparison differences and exact
native vertices remain diagnostic evidence. The rectangular locus agrees, but
parameter equivalence is unfinished. Broader curved/B-rep input qualification,
hatches/section styles, native junction/seam policies and performance remain open.

The preceding [restricted BlockEdit close choices](../../commands/block-edit.md) add
four document tests, one command test, one egui pointer test, one replay test and
four Python tests. Native Yes releases hidden/locked members with their world
geometry and flags; No normalizes them and retains their prototype membership;
Cancel returns to the editor. A default local save requires an explicit decision
and does not mutate the workspace. All nine native workflows match 45 model states with zero sampled numeric
difference
including mixed restrictions and all-members release. When no members remain,
Rhino retains the prior definition while accepting released model objects; the
local staged close follows that rule. Context navigation requires normalizing
restricted members first; its native warning behavior is not newly qualified.
Local Undo restores the model and released object/group state together. Broader
member types, layer-level restrictions, linked editors and native history/panel
fidelity remain open; no performance or GPU measurements were made.

The preceding [nested BlockEdit](../../commands/block-edit.md) adds six document tests,
one command test, one egui tree test, one native replay test and eight Python
tests. The tree follows validated member paths in composed instance frames;
ancestor geometry remains protected. Context changes stage geometry/catalog
checks atomically and have workspace Undo/Redo. Leaving a context accepts it;
Discard restores the latest accepted context checkpoint, while no-navigation
Discard retains the original full-document/redo behavior. Local model Undo can
restore the original pre-editor document after checkpoint acceptance.
All nine semantic native workflows match 48 model states with zero sampled
numeric difference for child editing, parent/child
returns, save/discard, translated/reflected placements, repeated references and
deeper nesting. Nine repeated captures show unstable ancestor member order for
identical inputs. The explicit context comparison consistently permutes member
records and leaf-path indices while keeping geometry samples, attributes, groups
and placements intact; raw records remain unchanged. Four additional frame
captures qualify the ordering observation. Moving a nested reference can remove
its row from Rhino's live dialog; that UI limitation is retained as a diagnostic.
The local validated tree still follows its reference placement. Native context
history, wider member types, nonuniform navigation behavior and performance
remain unqualified. The UI tree is cached by document history revision.

The preceding [BlockEdit input](../../commands/block-edit.md) adds five application
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

The preceding [BlockEdit controls](../../commands/block-edit.md) add six document tests,
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

The preceding [BlockEdit](../../commands/block-edit.md) adds seven document tests, one
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

The preceding [BlockResetScale](../../commands/block-reset-scale.md) adds six document
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

The preceding [AddObjectsToBlock](../../commands/add-objects-to-block.md) adds four kernel
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

The preceding [ReplaceBlock input](../../commands/replace-block.md) adds six GUI tests,
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

The preceding [ReplaceBlock workflow](../../commands/replace-block.md) adds two document
tests, two command tests, one GUI test, one replay test and two Python validation
tests. Six private-Xvfb native workflows match all 37 states for selected-only
and All scope, protected peers, reflected nonuniform placements, root metadata
and groups, and unchanged nested definition references. Replacement retains root
IDs and placements but clears geometry-attached root text. Local tests constrain
atomic preparation, protected explicit selection and Undo/Redo. Instance picking
and the definition chooser are supplied by the newer input milestone above;
layer-level protection, native option memory, linked definitions and performance
remain open.

The preceding [unique-definition workflow](../../commands/create-unique-block.md) adds
three document tests, two command tests, two GUI tests, one replay test and two
Python validation tests. Five private-Xvfb native workflows match all 30 states
for placement, root identity/metadata/groups, shared child definitions, subsequent
child redefinition and unused duplication. Copied prototype groups are cleared.
Immutable member geometry stays shared until later edits; rebinding and catalog
creation form one Undo step. Native name defaults, protected/group selection,
linked definitions, panel duplication interaction and performance remain open.

The preceding [BlockManager](../../commands/block-manager.md) adds three document tests,
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

The preceding [block group follow-up](../../block-groups.md) adds three document tests,
one oracle unit test, one replay integration test and four Python tests. All 16
native command workflows, including 82 whole-model states, match their geometry,
catalogs, ordered group memberships and group tables. Block clones top memberships
into fresh prototype groups; Explode copies them again, while ExplodeBlock reuses
leaf groups and discards nested-container/root memberships. GroupOutput creates
one extra group per root in a batch. SDK construction strips group indices; its
14-case replay retains eleven diagnostics and three matches, without execution
failures. Local tests independently constrain group allocation, rollback and
Undo/Redo. Native option memory, protected-root selection and history remain open.

The preceding [block workflow oracle](../../block-workflow-oracle.md) adds three oracle unit
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

The preceding [block expansion](../../commands/explode-blocks.md) adds five document tests,
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

The preceding [structural 3DM path](../../three-dm-structural-blocks.md) adds six I/O tests
and two command tests. Native definitions, member UUID lists and instance records
retain supported embedded graphs across export and editable import. The legacy
flattened reader independently agrees on placed point/line/cloud/B-rep geometry;
unit conversion preserves each linear map and scales every translation once.
Tests cover hidden members, hidden/locked roots, malformed graph rollback,
existing-destination preservation, shared redefinition and isolated import name
conflicts. Structural reading rejects linked definitions and unsupported members;
prototype object-lock modes reject export. Broader definition/model metadata and
fresh Rhino cross-reader/performance evidence remain unverified.

The preceding [block object snapping](../../block-snapping.md) adds five drafting tests
and two CPU viewport tests. Existing feature and intersection queries now use
immutable member records with independent cache IDs and root result ownership.
Point mode also includes root/nested insertion points, including empty nested
references. Tests cover all existing modes, visibility/locking, repeated
placements, lazy-cache reuse, definition edit/Undo/removal and mesh intersections
inside one instance. Four-view cursor checks confirm Point/End ownership and
mesh-wire admission. Ordinary documents avoid an expanded-source vector;
suspended queries remain cold. The full legacy snap suite passed alongside these
checks. No fresh native block-snap capture, GPU run or timing comparison was made.

The preceding [Block/Insert commands](../../commands/blocks.md) add five document creation
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

The preceding [instance objects](../../block-instances.md) retain definition relationships
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

The preceding [native block catalog](../../block-definitions.md) adds 12 regressions for
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
remain unverified. See [scope](../../three-dm-blocks.md).

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
See [scope](../../three-dm-blocks.md).
