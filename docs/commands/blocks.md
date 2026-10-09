# Block and Insert

`Block` captures selected objects as a shared definition and replaces them with
one instance at the chosen base point. `Insert` places another instance of an
existing definition. Definition edits update every reference, including nested
instances. [Instance objects](../block-instances.md) retain their relationship
through affine edits, copies and history.
The [source provenance](../block-command-provenance.json) distinguishes documented
rules, local policies and completed checks.

```text
Point 11,22,33
SelAll
Block 10,20,30 "Part A"
Insert "Part A" 10,20,30 Scale=2,3,4 Rotation=90
```

The original point becomes `(1,2,3)` in definition coordinates. The first
instance restores `(11,22,33)`; the inserted instance places it at `(4,22,42)`.
This follows McNeel's documented [base-point normalization](https://developer.rhino3d.com/en/guides/cpp/creating-blocks/).

## Block

The complete script form is `Block base-point block-name`. A base point can use
the ordinary comma-separated or three-scalar coordinate syntax. Quote names
containing spaces or command keywords. Names retain internal whitespace and use the catalog's
ASCII case-insensitive lookup.

For interactive use, run `Block`, select objects if needed, and press Enter.
Pick or type the base point, then enter the name. With existing preselection,
the command starts at the base-point getter. `Block x,y,z` also starts the name
getter when sources are already selected. Escape cancels without changing the
model or its history.

Members retain raw object attributes, geometry user text and ordered group IDs.
Capturing at the origin shares the immutable source geometry. Geometry text is
copied directly, preserving existing raw keys/values without setter normalization.
Grouped members retain their metadata inside the definition; the new root is
ungrouped and uses current-layer default attributes. Instance sources become
nested references instead of independent placed leaves. Group definitions used
inside blocks cannot be deleted through the low-level group-removal API.

Reusing a name updates its existing definition ID and every existing placement,
as described in the [Block help](https://docs.mcneel.com/rhino/8/help/en-us/commands/block.htm).
Cycles, unavailable sources, invalid groups or failed normalization/placement
reject the entire edit. Successful creation is one Undo step; replay restores
source order, geometry storage, attributes and group memberships. The resulting
root is currently unselected, a local selection policy without a fresh native
command capture.

## Insert

```text
Insert block-name point [Scale=n|x,y,z] [Rotation=degrees] [Axis=x,y,z]
```

`Scale` defaults to `1,1,1`; `Rotation` defaults to zero; `Axis` defaults to World Z.
Scale applies in definition/world XYZ axes, followed by rotation and translation
to the insertion point. Rotation uses the shared OpenNURBS cardinal-angle cleanup,
avoiding amplified off-axis noise at extreme scales. Negative nonzero scales permit reflection; zero scales
and zero axes are rejected. `Rotate` aliases `Rotation`, with duplicate aliases
rejected. These explicit world-axis rules follow the [InsertBlock API](https://developer.rhino3d.com/api/rhinoscript/block_methods/insertblock.htm);
other Rhino Insert workflows remain unverified.

Run bare `Insert` to list available definitions, enter a name and pick its insertion point, or
start with `Insert "Part A"`. Before picking, type `Scale=...`, `Rotation=...` or
`Axis=...` to update options atomically. Typed and picked points share the existing
point-input/filter machinery. New instances use current-layer default attributes
and are unselected; existing preselection is retained. Insertion is one Undo step.
Invalid options, missing definitions and failed later placements leave the model
and redo branch unchanged.

## Boundaries and verification

These commands currently handle definitions already in the document. External
file insertion, linked blocks, descriptions/hyperlinks, native option memory,
definition-editing tools and broader file metadata remain unfinished.
Supported embedded definitions/references now have [structural 3DM round trips](../three-dm-structural-blocks.md);
linked/external definitions and local prototype lock modes remain unsupported.
Visible members and insertion points now participate in [object snapping](../block-snapping.md).
Use [Explode/ExplodeBlock](explode-blocks.md) to return editable members one level or recursively.

The documented layer-lock rule is implemented: locking a member's layer does
not lock the containing instance. Root object/layer locking remains effective;
member object-mode locking is a local display rule awaiting a native capture.

```sh
cargo test --release --workspace block
```

Tests cover base normalization, nested capture, group/text metadata, redefinition,
source replacement/history, noncommuting insertion transforms, quoted names,
invalid options, cycles, late failures and interactive cancellation. No fresh
Rhino block-command, GPU-backend or performance comparison is claimed.
