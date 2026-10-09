# 3DM block geometry import

[File formats](file-formats.md) · [OpenNURBS fixture producer](../crates/viboceros-io/tests/fixtures/instances/generate.cpp)

The low-level flattened 3DM reader expands supported block references into independent placed
geometry. Definition objects are excluded from ordinary model objects, so their
unplaced prototypes do not appear at the origin. Repeated references remain
separate occurrences, and nested references compose in child-to-parent order.
Editor Open3dm/Import3dm now use the separate [structural reader](three-dm-structural-blocks.md)
to retain editable relationships. These notes describe the preceding flattened API.

The OpenNURBS bridge resolves definition/member IDs and copies unmodified
geometry records plus the placement chain. The Rust geometry kernel validates
finite affine matrices, composes them with checked arithmetic and applies the
result to native geometry. Physical unit conversion follows placement, including
translations. B-rep matching tolerance is resolved in source units; primitives
retain numerical validation independent of document feature size.

Members retain names, source layers, geometry/attribute user text and existing
group memberships. Parent group memberships are inherited. OpenNURBS resolves
ByParent colors without changing member layers. Parent object/layer visibility
and locking propagate to placed members. Ordinary model objects retain their
preceding decode path. Point-cloud member channels and editable B-rep surfaces,
edges and UV trims use the existing codecs and native transforms.

Traversal limits nesting to 64 definitions and expansion to 100,000 leaf objects
per top-level object. A separate one-million-visit ceiling also bounds branching
graphs of empty definitions, whose work is not bounded by the leaf count.
Cycles, missing definitions/members and invalid placements
fail reading before document edits. Unsupported native geometry suppresses its
entire top-level occurrence and increments the existing unsupported-object count.
A native member that fails Rust geometry decoding/placement fails the file rather
than returning a partial block. External linked files are not opened; only geometry
present in the archive can expand.

The imported representation is flattened: block names/definitions, instance
identity and definition-editing relationships are not retained. Parent-only user
text is not merged into member text. Export writes the resulting independent
objects, as the import report states. Native Block/Insert/editing commands and
full block metadata round trips remain to be implemented.
The separate [native document catalog](block-definitions.md) now provides shared
definitions, nested references and transactional editing, but this importer does
not yet populate that catalog or create editable instance objects.

Independent OpenNURBS-generated fixtures cover nested/repeated references,
nonuniform scale followed by rotation/translation, inherited color/groups,
hidden locked members, point-cloud colors and a box B-rep of placed volume 24.
The placement audit also checks reflected solid boundaries/volume, invalid
projective/singular maps and an acyclic empty-definition graph that exceeds the
visit budget. Stored point-cloud normals retain the OpenNURBS transform policy.
Tests check unit conversion, atomic failure/redo, subsequent export/reimport and
absence of unplaced prototypes. The producer validates ordinary fixture geometry
and definition records; separate fixtures intentionally contain a missing reference
and a cycle. These are original public-library test data, not Rhino captures.
Fresh Rhino cross-reader, broader linked-definition and performance audits remain
unverified. See [source and fixture provenance](three-dm-block-provenance.json).
The [traversal audit provenance](three-dm-block-audit-provenance.json) preserves the
preceding sources and records the new failure/placement fixtures.
