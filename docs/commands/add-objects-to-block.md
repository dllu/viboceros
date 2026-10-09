# AddObjectsToBlock

`AddObjectsToBlock` picks an embedded block instance, then objects to add. A
single preselected instance skips the target getter. Select source objects by
clicking or using a selection window; Enter accepts, and Esc or Cancel restores
the selection from before the command. Repeating the command cancels the old
getter before starting a new one. Picking and cancellation preserve the model
and redo branch. An empty source set or a failed edit keeps the getter open.

Scripted forms identify the target and optional sources by object ID:

```text
AddObjectsToBlock target-object-id source-object-id ...
AddObjectsToBlock target-object-id
```

The second form uses selected sources in the registry API; the UI opens the
source getter. The target is excluded from source selection. All explicitly
identified objects must be editable. Sources are ordered as they occur in the
document, converted into the selected instance's local frame, appended to its
definition and removed from the model. Nested instances stay references.

Every use of the definition refreshes, including protected peers and references
inside other definitions. Definition IDs, existing root IDs, placements,
attributes, attribute text, geometry text and model groups remain intact. Added
members retain raw attributes and geometry text. Circular inputs use the native
command's circle-fitting policy under nonuniform placement, while general affine
editing retains exact conics through NURBS conversion. The command preserves the
original curve parameter domain. This compatibility policy follows public
[OpenNURBS circle transforms](https://github.com/mcneel/opennurbs/blob/8.x/opennurbs_circle.cpp)
through a Rust adaptation in `third_party/opennurbs-rust` with retained notices.
All target prototype memberships are
cleared, including existing members' groups; unused model group records remain.

Preparation validates permissions, geometry, group consistency, graph cycles,
depth and resource limits before mutation. Adding a target to itself or adding an
ancestor definition rejects atomically. The definition update and source removal
form one Undo entry, with full catalog, object and group replay.

The document API is `add_objects_to_block`. It uses the kernel's
`AffineTransform3::try_inverse`, which computes the determinant, inverse
coefficients and inverse translation from exact rational representations of the
input doubles. Each output coefficient is rounded once. Exact singularity,
overflow and loss/reversal of the rounded inverse's orientation reject. This
does not certify round-trip accuracy for arbitrarily ill-conditioned placements.

Eight Rhino 8.32.26160.13001 workflows captured under private Xvfb constrain
source consumption, reflected nonuniform placement, nested references, existing
and added groups, root/member metadata and circle/arc fitting. Local tests cover
extreme affine coefficients, cancellation, graph rejection, protected objects,
and Undo/Redo. See [source and capture provenance](../block-add-provenance.json).

Linked definitions, native mouse/group picking and selection/history parity,
layer-level protection, general curved B-rep comparison, GPU rendering and
performance remain unverified or unfinished. The current scripting syntax uses
explicit IDs; the native interactive getter remains the behavioral reference.

Reference: [AddObjectsToBlock](https://docs.mcneel.com/rhino/8/help/en-us/commands/block.htm#AddObjectsToBlock).
