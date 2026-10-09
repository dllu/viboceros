# Explode and ExplodeBlock on instances

`Explode` expands block instances one level: geometric members become ordinary
objects and nested references remain editable instances. `ExplodeBlock` recursively
expands nested references into placed geometric members. This follows the
[documented component behavior](https://docs.mcneel.com/rhino/8/help/en-us/commands/explode.htm).
Annotations/text fields and linked blocks remain outside the current model.
The [source provenance](../block-explode-provenance.json) records the implemented
contract, local policies and verification evidence.

```text
Point 1,2,3
SelAll
Block 0,0,0 child
SelAll
Block 10,0,0 parent
SelAll
Explode
Undo
SelAll
ExplodeBlock GroupOutput=Yes
```

The first expansion returns an instance of `child`; the recursive expansion
returns the point at `(1,2,3)` with an output group. Definitions remain in the catalog.

## Picking and options

`ExplodeBlock [AllBlocks] [GroupOutput=Yes|No]` uses selected instance roots by
default. Bare command-first use opens an instance-only selection prompt. Set
GroupOutput while picking and press Enter. AllBlocks can be typed in that prompt
to finish without picking, or included in a complete command.

AllBlocks enumerates every model instance root and applies the same preparation
permissions to the batch, rather than skipping protected roots. Selected restricted
group peers follow the existing Explode policy: outputs are created and the original
is retained unselected. Other permission failures abort the batch. This is a local
policy without a fresh native AllBlocks capture.

GroupOutput defaults to No. Yes adds a separate group per root after prototype/root
memberships. Native option memory and exact multi-root grouping/selection remain
unverified.

## Geometry, metadata and groups

Geometry uses checked placement or validated immutable placed snapshots. Recursive
expansion retains member curve/surface/mesh representations; one-level child
references remain instances. Source definitions are unchanged.

Members retain names, raw attribute/geometry user text, layers and wire density.
Visibility and object-lock flags follow current hierarchical display state.
ByParent colors become resolved explicit object colors when the container is
removed; other supported color sources remain unchanged. Root-only user text is
not merged into member text.

Prototype groups receive fresh definitions per root and nested placement scope.
Repeated uses of one definition cannot merge unrelated outputs into a shared
group. Member group order is retained, followed by the root's existing groups.

## Preparation, history and limits

Preparation validates source editability, placement, metadata and output budgets
without model edits. Commit checks source geometry/attributes/groups, relevant
layer states and tolerance. Stale plans reject without changing model or redo.
GUI picking/cancellation stays outside model history.

The command records one Undo step, including mixed ordinary Explode inputs.
Undo restores source IDs and geometry storage and removes outputs/copied groups;
Redo restores outputs. Exact result selection can retain restricted inherited
members without selecting untouched group peers. Source selection is consumed
before deletion, following the existing ordinary Explode adapter.

The aggregate command limit remains 1,000,000 outputs; graph/placement limits
apply separately. Empty instance geometry remains unsupported, including a direct
empty child requiring a new instance during one-level expansion. Recursive empty
subtrees produce no geometric members.

```sh
cargo test --release --workspace block
```

Regressions cover expansion depth, placed geometry, metadata, scoped groups,
restricted states, stale preparation, budgets, mixed history, grouping and GUI
picking/cancellation/AllBlocks. No fresh Rhino command, GPU or timing evidence is claimed.
