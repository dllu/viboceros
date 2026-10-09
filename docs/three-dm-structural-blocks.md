# Editable blocks in 3DM files

Open3dm and Import3dm now retain supported embedded block definitions, ordered
members, nested references and root instance transforms. Definition prototypes
remain outside the ordinary object table. Repeated references share a document
definition; imported definitions remain editable through the catalog API.
Export3dm, SaveAs and Save write native OpenNURBS definition/reference records.
The [source provenance](three-dm-structural-provenance.json) records the supported
graph contract, validation and remaining fields.

The [structural reader](../crates/viboceros-io/src/three_dm/blocks.rs) is distinct
from the existing [flattened reader](three-dm-blocks.md). Public
`read_3dm_file_with_blocks`, `read_3dm_file_with_blocks_model_tolerance` and
`read_3dm_file_with_blocks_in_units` expose the structural representation.
Existing `read_3dm_file` APIs retain independent placed geometry for callers that
need that representation.

## Records and validation

The file adapter represents definitions with names and member objects. References
use definition-table indices and checked finite nonsingular affine transforms.
The native bridge resolves archive UUIDs to those indices. The document adapter
allocates independent typed IDs, maps all forward references, and renames imports
when names conflict with existing definitions. Existing definitions are unchanged.

Both adapters reject missing definitions, cycles, singular/projective placements,
excessive nesting, duplicate names and invalid member ranges. Stored graphs are
bounded to 10,000 definitions, 100,000 total members and 64 levels. Document
placement retains its separate traversal/output budgets. Structural import fails
on missing/unsupported definition members; it does not silently drop part of a
definition. Unsupported ordinary geometry follows the existing reporting policy.

Members retain names, object/geometry user text, source layers, colors/color
sources, visibility, wire density and ordered group memberships. Root groups
remain attached to the root object. Native visibility and root locking are read
independently, preserving hidden/locked combinations. Prototype visibility is
restored after switching native definition-object mode for decoding/writing.

Native definition-object mode cannot represent the editor's local prototype
object-lock mode. Export rejects such members before replacing the destination;
source-layer locking and root locking remain supported. Linked/external
definitions are explicitly rejected by structural import. The flattened reader
retains its preceding boundary for geometry present in the archive.

## Export and units

Export prepares definition members with the existing geometry codecs, including
full-order curve decomposition. Native member IDs and definition IDs are created
once per file; member UUID lists and instance references use those exact IDs.
Definition geometry uses native definition-object mode and never appears as
ordinary geometry. Definition/reference bounding boxes use finite display/control
bounds and checked transformed corners; they are not tight-bound certificates.

All preparation/validation precedes committing the staged destination. Failed
encoding or native writing preserves the previous file. Curve adaptation may
expand a definition member into multiple native objects without changing the
in-memory document; write reports count ordinary model objects separately.

Unit conversion scales definition geometry and every local/root translation once,
retaining reference linear coefficients. Model/view/layer/group conversion keeps
its existing behavior. Import stages catalog creation and object insertion in one
document transaction; failure restores layers, groups, objects, catalog and redo.
Open swaps in a fresh document only after complete success.

## Verification and remaining scope

```sh
cargo test --release --workspace block
```

OpenNURBS fixtures and native bridge round trips check prototypes, nested and
repeated references, noncommuting placement, source metadata, hidden/locked state,
physical unit conversion and B-rep vertices/volume. The separate flattened reader
checks placed geometry from native structural exports. Command tests verify
editable shared definitions, name-conflict isolation, model/history replay and
malformed graph rollback.

These are local/public-library tests. No fresh Rhino cross-reader or performance
capture is claimed. Definition descriptions, hyperlinks, per-definition unit
metadata, linked/external file management, materials and other unsupported model
resources are not preserved by the editor's current file model. This establishes
the supported definition/reference graph round trip, not complete 3DM fidelity.
