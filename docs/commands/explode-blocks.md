# Explode and ExplodeBlock on instances

`Explode` expands block instances one level: geometric members become ordinary
objects and nested references remain editable instances. `ExplodeBlock` recursively
expands nested references into placed geometric members. This follows the
[documented component behavior](https://docs.mcneel.com/rhino/8/help/en-us/commands/explode.htm).
Annotations/text fields and linked blocks remain outside the current model.
The [original provenance](../block-explode-provenance.json) records the initial
contract and local policies. The [block workflow oracle](../block-workflow-oracle.md)
adds fresh native command evidence and the resulting metadata corrections.

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

GroupOutput defaults to No. Yes adds a separate group per root after retained
leaf memberships. A two-root native capture confirms separate output groups.
Native option memory and broader multi-root selection remain unverified.

## Geometry, metadata and groups

Geometry uses checked placement or validated immutable placed snapshots. Recursive
expansion retains member curve/surface/mesh representations; one-level child
references remain instances. Source definitions are unchanged.

Members retain names, raw attribute user text, layers and wire density.
Geometry text is retained except when an analytic circle/arc becomes a NURBS
curve under a nonsimilarity placement; Rhino commands create that converted
geometry without its original geometry strings. Existing NURBS members keep theirs.
Visibility and object-lock flags follow current hierarchical display state.
Raw ByParent colors remain ByParent, following the native commands; the SDK
explosion API instead resolves explicit object colors. Root-only user text and
geometry text are not merged into member text.

One-level `Explode` copies each direct member's top prototype group into a fresh
model group. Recursive `ExplodeBlock` keeps the geometric leaf's prototype groups
and ignores groups on nested reference containers. Repeated or sequentially
exploded instances can therefore join the same leaf group. Both commands discard
the root's own memberships; untouched group peers remain in their original group.
See the [native capture and replay](../block-groups.md).

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
picking/cancellation/AllBlocks. Twenty-two earlier native command workflows constrain
placed geometry, definitions and supported metadata. Sixteen additional workflows
constrain construction and explosion group behavior. Protected-root selection,
option memory, GPU behavior and performance remain unverified here.
