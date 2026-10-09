# Native document instance objects

The document object table supports `Geometry::BlockInstance`. Each instance stores
a typed definition reference and affine placement; its placed leaves and bounds
are derived immutable geometry. Instance insertion creates one object ID, so
selection, attributes, geometry user text, groups, deletion and history apply to
the root object. The [definition catalog](block-definitions.md) remains shared.
The [source provenance](block-instance-provenance.json) records the local policy
boundary and verification evidence.

`Document::add_block_instance`, `add_block_instance_with_attributes` and
`block_instance_geometry` create instances from the current catalog. All ordinary
geometry admission, replacement and copy paths revalidate instance values against
the receiving document, refreshing stale derived geometry and rejecting foreign
or missing definitions. Definition members store nested instances as references;
embedding an instance value as a geometric leaf is rejected.

## Editing and history

Catalog edits stage every live instance before mutation. Changed instances receive
new immutable geometry snapshots; unchanged instances retain their snapshots.
The catalog and affected instance snapshots exchange together during Undo/Redo
and transaction rollback. Removing a definition used by a live instance, removing
its last geometric member or causing a later placement to fail rejects the entire
edit. Catalog changes refresh locked/hidden instances as well as ordinary ones.

Affine document edits compose the placement and resolve geometry from the
definition. Copies retain the shared definition relationship, with the ordinary
attribute/group policies. A later definition edit updates both original and
transformed copies. Singular placements are rejected before document mutation;
reflections remain supported. Bounds aggregate member bounds; tight bounds use
each member's tolerance-controlled query.

Rescaling units scales definition geometry and every nested/root translation
once, retaining reference linear coefficients. Root instances are resolved against
the staged scaled catalog. Both tables complete all fallible work before commit.
History restores the original catalog and geometry storage. Metadata-only unit
changes retain numeric geometry.

## Display and selection

The viewport builds cached display data for each derived leaf and renders the
instance through the existing point/cloud, curve, surface, B-rep and mesh paths.
Wireframe, shaded and ghosted modes share these caches. Affine previews apply to
member display data while retaining root selection identity.

The hierarchical display policy intersects visibility along the root-to-leaf
path, including source layers. Locking a member's layer does not lock the instance,
following the [documented layer-lock rule](https://docs.mcneel.com/rhino/8/help/en-us/commands/block.htm).
Root object/layer locking remains effective; member object-mode locking is a local
display rule awaiting a native capture. ByParent colors
inherit the nearest resolved parent color; object and layer colors use their
existing document rules. Material colors retain the existing layer fallback.
Raw definition/member metadata is unchanged. Display resolution builds one
definition/layer lookup index per instance, rather than scanning both tables for
every leaf. This policy has local regression evidence, not a fresh Rhino capture.

Click capture uses visible member points, curves and shaded faces, returning the
root object ID. Window, crossing, fence, lasso and boundary queries aggregate
visible member primitives. Definition edits, layer visibility and parent color
changes invalidate the applicable geometry or scene caches. Root grips are not
offered.

## Remaining integration

[Block and Insert](commands/blocks.md) now provide command-driven creation and
placement. [Object snapping](block-snapping.md) queries visible members and
insertion points while retaining root ownership. Definition-editing tools, subobject editing and
broader file metadata remain unfinished. Supported embedded definitions and
references now have [structural 3DM interchange](three-dm-structural-blocks.md).
[Explode/ExplodeBlock](commands/explode-blocks.md) return editable members one
level or recursively, retaining metadata and isolated prototype groups.
Morphing, defining-point extraction, shape-duplicate comparison and command
adapters that require ordinary curves/surfaces/meshes explicitly reject unsupported
instance inputs. Empty block instance geometry is currently unsupported, although
empty definitions remain valid catalog entries. No fresh native block-command,
graphics-backend or performance comparison is claimed.

## Verification

```sh
cargo test --release -p viboceros-document block_instances::
cargo test --release -p viboceros-command block_instance_tests
cargo test --release -p viboceros --bin viboceros viewport::scene::block_tests
```

Document regressions cover shared edits, stale/foreign values, root identity,
metadata/groups, history storage, reflections, nested unit scaling and atomic
failure. Command tests check Move retains the relationship and unsupported export
preserves both destination and document. CPU scene tests compare mixed instance
members with independent placed geometry across four views, three modes and
selected/unselected states; they also check root picking, window selection,
visibility and cache replay. They do not execute the GPU backend.
