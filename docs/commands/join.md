# Join and JoinCopy

[Command reference](README.md) · [Curve joining details](../curve-editing.md)

```text
Join
Join JoinDisjointMeshes=Yes
Join JoinDisjointMeshes=No
JoinCopy
JoinCopy JoinDisjointMeshes=No
```

`Join` and `JoinCopy` accept curves, polygon meshes, or surfaces/polysurfaces.
Each invocation uses one family; natural NURBS surfaces and B-reps can mix.
The command-first workflow filters eligible objects and exposes the remembered
mesh option. Mixed-family and SubD joining remain unimplemented. Surface joining
uses [certified boundary assembly](../brep-edge-joining.md) with the coverage and
Rhino differences described below.

`JoinCopy` retains all original objects, including their exact geometry, IDs,
attributes, and groups. Outputs inherit their seed's attributes and memberships.
Preselection leaves originals and outputs selected. Command-first surface/mesh copying
and early closed-curve copying retain participating originals selected, with
outputs unselected; ordinary open-curve copying clears selection.
Neither command expands selection to unselected group peers.
Disconnected curves and single selected objects are not duplicated. With
multiple meshes and `JoinDisjointMeshes=No`, even isolated mesh components
produce fresh copies. Both commands use the same staged geometry/document path.

## Surfaces and polysurfaces

Open inputs use a certified distance strictly below 1.8 times the document
tolerance for preselection, or 2.1 times for command-first selection. These
[measured command policies](../join-selection-distance.md) do not change the
explicit kernel assembly API's distance argument; exact-cutoff endpoint behavior
still has recorded differences.
Complete boundaries must pass the kernel's whole-curve certificate. Different
curved degrees, knot refinements and positive rational weights now have an
[exact span/product certificate](../join-curved-certificates.md) through degree 16;
[projective parameter alignment](../join-projective-correspondence.md) also
certifies changed rational speeds. Continuous, exactly straight boundaries
support independent degrees/parameter speeds
and automatic partial-overlap splitting. Surfaces are never refitted. Newly
closed nonzero-volume outputs are oriented outward. This is not a Boolean union.

Preselection processes document order and emits every connected output, including
fresh copies of unjoined open inputs. A disconnected source can contribute to
multiple outputs, each inheriting its earliest contributing source's attributes
and groups. Command-first picks reconsider the original accepted boundaries in
pick order. Unrelated picks, picks that eliminate every cross-source join, and
picks after complete closure are skipped without retry. A contacting pick can
remain a separate output after boundary competition is resolved; all output
pieces use the initial seed's attributes. Enter completes selection, even after closure.
JoinCopy retains participating originals selected and leaves these outputs
unselected; ordinary command-first Join deselects its outputs and skipped picks.
Closed inputs are ignored and deselected. An all-closed selection, one open
input, or command-first picks with no connection fail without geometry/history
edits. The latter retains only the first open seed selected.

The [66 matching command cases](../../tools/rhino_oracle/fixtures/join_surfaces.json)
and [raw Rhino records](../../tools/rhino_oracle/observations/join_surfaces.json)
cover both commands and selection modes, adjacent/disconnected inputs, seeded
pick order, outward closure, rational/closed boundaries, duplicate sheets,
multi-component sources, source retention, layers/colors/groups, and unselected
peers. Every recorded field is compared at `1e-10` absolute / `1e-12` relative
epsilon, without geometry, domain, or ordering normalization. Inputs are shared
3DM artifacts, checked after native roundtrip and Rhino document insertion.
The [initial live comparison](../join-surfaces-comparison.json) records fixture,
observation, and release-binary hashes; maximum numeric residual is below `2.04e-12`.

The original [26-case discrepancy archive](../../tools/rhino_oracle/fixtures/join_surface_differences.json)
and [raw records](../../tools/rhino_oracle/observations/join_surface_differences.json)
now has four fully resolved partial-overlap and sixteen resolved gap cases. Its duplicate-wall cases now
have the correct two outputs, raw edge order, retained domains, and document
state; only the zero-volume shell's face senses differ. Remaining policies are:

- Native retains the first face sense for a zero-volume double sheet. Rhino's
  chosen sense differs for the recorded duplicated vertical walls.
- [Spatial boundary rebuilding](../join-gap-rebuilding.md) now matches the
  recorded ordinary gaps, including incident curves and component tolerances.
  Selection-dependent distances now match the recorded `0.002` gap at document
  tolerance `0.001`: joined command-first, unjoined preselected. At the archived
  translated `0.0021` cutoff, command-first Rhino creates two unjoined copies
  and moves only their top corners, while native joins them. The complete raw
  discrepancy remains in the tests.

The [boundary-matching audit](../join-boundary-matching.md) adds 160 cases:
148 now match every recorded field; 12 retain zero-volume orientation differences.
It covers partial/crossing overlaps, opposite edge
directions, source-order permutations, three-way competition, near competitors,
overlaid walls, and already-joined input polysurfaces. UV trims in the original
partial-overlap records differed only by roundoff within the ordinary epsilon;
the earlier description of a separate UV-domain policy gap was incorrect.

The [gap-rebuilding audit](../join-gap-rebuilding.md) adds 108 cases: 98 fully
match, two expose transitive-cluster selection differences, and eight have area
integration differences despite matching every other field. Independent
high-precision integration supports the native areas. The
[selection-distance and clustering audit](../join-selection-distance.md) adds
150 cases, including every discovery discrepancy. The
[corner-only follow-up](../join-corner-clustering.md) adds 84 cases that isolate
endpoint movement from edge mating. The
[curved-certificate audit](../join-curved-certificates.md) adds 48 cases: 40 full
matches, four redundant outer-knot interchange differences and four larger native
uncertainty bounds. The previous 594 records retain exactly their results.
The [projective-correspondence follow-up](../join-projective-correspondence.md)
adds sixty records: forty full matches, eight area-only differences, four
uncertainty differences, four short-overlap orientation failures and four
unsupported non-projective correspondences in its initial audit. The
[short-overlap follow-up](../join-short-overlaps.md) resolves all four orientation
failures and adds 102 records, of which 78 fully match. Complete short edges
retain their distinct endpoints; tiny crossing overlaps within the tip-contact
radius do not produce spurious cut-edge mates. The
[partial-boundary certificate audit](../join-trim-certificates.md) resolves
eighteen more uncertainty-only differences and adds eighty pre-split curved
records, all retaining raw edge-cleanup or representation differences.
In total, 651 of 884 cases fully match, with 32 native execution failures and 201
other differences. Redundant seam coalescing, general surface-image uncertainty,
offset cut ownership and ordered clustering remain incomplete. This is not general parity.

These are scoped observations, not general threshold rules. Generic curved
partial overlaps, non-projective curved correspondence, nested/cavity-solid classification,
and arbitrary ambiguous matching remain unsupported or unproven. Boundary search
is bounded; command-first joining currently rebuilds the growing assembly at
each attempted pick. No kernel speedup is claimed from command probe timings.

## Curves

Preselection scans document table order and batch-joins all compatible chains,
using majority direction and chord-length parameters for wholly linear batches.
Individual command-first picks extend only the first open curve, in pick order;
skipped curves are not revisited and a second unrelated chain is not started.
The seed's direction and usually its parameter interval are retained in this mode;
[mixed linear-run consolidation](../join-encodings.md) has a distinct domain policy.
An individually picked chain completes immediately on closure, without Enter.
See [cycle seams and command-boundary evidence](../join-cycles.md).
See [curve joining details](../curve-editing.md) for endpoint matching.
The [one-pass seeded matcher](../seeded-join.md) bounds connectivity work by pick
count, without constructing candidate pairs between unrelated unconsumed sources.

A single object fails without geometry edits. Disconnected preselected open
curves succeed unchanged; individually picked curves that cannot extend the seed
fail and leave only the seed selected. Neither case changes undo/redo history.
Existing closed curves are ignored when open curves are present; an all-closed
selection fails and is released without changing geometry or undo/redo history.

## Meshes

`JoinDisjointMeshes=Yes` (the native default) creates one mesh, preserving raw
vertex order, coincident-but-unwelded vertices, unused vertices, polygon types,
face order, and winding. This is not a Boolean union. Mesh alignment can move
naked vertices before concatenation, as described below.

`No` emits connected source components. It compacts raw vertices into first
face-use order and orients adjoining source meshes. A shared vertex can connect
objects even without a shared edge; a vertex-only connection reverses the later
mesh's winding in the recorded cases. Already-disconnected source meshes remain
separate, even when another source bridges their pieces. Ordinary disconnected
inputs are also replaced with fresh IDs. A single selected mesh fails unchanged,
retaining its selection.

The earliest source supplies each output's layer, name, color, and group
memberships. Preselection uses document table order; command-first selection
uses pick order. Preselected results stay selected; command-first results are
deselected. Unselected group peers are not reselected. All results are staged
before mutation and form one undo step; failures preserve the previous redo
entry. Options live in the command registry, outside model history.

## Alignment policy

The [Rhino command documentation](https://docs.mcneel.com/rhino/8/help/en-us/commands/join.htm)
describes alignment of naked mesh vertices. The recorded Rhino 8.32 commands
use a narrower matching distance: `absolute document tolerance * 1e-4`, tested
with document tolerances from `1e-6` to `1`. Matching uses finite binary32
positions, while outputs retain the original binary64 anchor coordinates.
This explains both threshold-boundary rounding and movement larger than the
distance when translated positions coincide in binary32. Native kernel callers
choose the matching precision explicitly; the command selects this measured
Rhino policy.

Interior vertices can anchor naked neighbors without moving themselves.
Alignment is not transitive clustering. Interior anchors are processed first,
then naked anchors in source order.
An unassigned later anchor can reclaim a neighbor assigned to an earlier anchor.
Each moved vertex matches its final anchor directly in the chosen precision.
This avoids moving an entire chain to a remote endpoint. Distinct raw vertex
indices are not welded together.

## Architecture and validation

The command's `join` module separates curve/mesh/B-rep geometry staging from shared
copying, deletion, direct selection, and preferences. Kernel `mesh/append` performs checked linear
concatenation; `mesh/join` handles matching, connectivity, and orientation.
Candidate matching uses a widest-axis sweep, with 100,000 inputs, one million
matching pairs, and 16 million candidate scans as resource ceilings. Exhausting
a budget or producing a degenerate face fails atomically. Dense spatial queries
can still reach the work ceiling; this is not an end-to-end speed comparison.

The [181-case fixture](../../tools/rhino_oracle/fixtures/mesh_join.json) and
[raw Rhino observations](../../tools/rhino_oracle/observations/mesh_join.json)
cover adjacent/disjoint meshes, triangle/quad mixtures, winding, non-manifold
contacts, unused vertices, disconnected sources, chains, translated coordinates,
interior anchors, tolerance/precision thresholds, reversed selection order, and
pre/postselection. Both engines receive identical double-precision vertices and
face indices, checked before and after Rhino document insertion. Replay compares
every recorded field exactly, without topology, coordinate, or order normalization.
All 181 cases match exactly (maximum numeric error zero). See the
[comparison report](../mesh-join-comparison.json), which records input,
observation, and release-binary hashes.

Independent tests check concatenation arithmetic, 10,000 appended meshes,
2,000 disconnected outputs, 256 randomized alignment cases against a literal
all-pairs reference, bounded direct movement, input preservation, groups,
two undo/redo cycles, error rollback, and the app's object-selection workflow.

The additional [140-case workflow fixture](../../tools/rhino_oracle/fixtures/join_workflow.json)
and [raw observations](../../tools/rhino_oracle/observations/join_workflow.json)
check both commands, both selection modes, source identity/retention, creation
order, layers/colors/groups, no-ops, closed inputs, independent chains, NURBS/line
joins, unrelated nonlinear curves, and raw local/parent parameter domains.
They compare every recorded field, with absolute epsilon `1e-10` and relative
epsilon `1e-12` for numbers; mesh indices and document metadata agree exactly.
See the [workflow comparison](../join-workflow-comparison.json).
Unit tests also verify exact source preservation, group reverse indices, atomic
undo/redo over two cycles, and app command-first picking for both commands.

```sh
tools/rhino_oracle/run_headless.sh compare tools/rhino_oracle/fixtures/mesh_join.json --timeout 420
cargo test --release -p viboceros-oracle join_command::tests
tools/rhino_oracle/run_headless.sh compare tools/rhino_oracle/fixtures/join_workflow.json --timeout 360 --relative-epsilon 1e-12
```

Normals, texture coordinates, per-vertex colors, ngons, restricted-source
deletion behavior, and arbitrary conflicting orientation cycles need further
coverage or implementation. Inputs outside finite binary32 coordinate range
retain binary64 matching natively; Rhino parity for that range is unproven.
These observations establish the recorded cases, not full mesh compatibility.

The four earlier closed-chain discrepancies are resolved in the additional
[284-case cycle suite](../join-cycles.md). Measurements now capture the named
command's EndCommand result and state, not subsequent macro commands.
A further [544 encoding and seam cases](../join-encodings.md) cover weighted NURBS,
polycurve consolidation, and representation-dependent seam/domain policies.
The [48 endpoint-search cases](../join-endpoint-search.md) ensure unrelated distant
sources do not hide nearby connections through spatial-index rounding.
Batch picks during a command, Undo-within-prompt, edge
subobjects, cross-command sharing of remembered options, and Rhino construction
history associations for JoinCopy still need independent coverage/implementation.
