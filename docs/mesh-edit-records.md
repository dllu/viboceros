# Stored mesh faces after grip edits

[Mesh topology](mesh-topology.md) · [Grip transforms](grip-transforms.md)

Mesh grip edits retain raw vertex identities, ordered triangle/quad face
records, colors and n-gon membership even when corners coincide or become
collinear. A subsequent edit can restore those faces. Quads stay quads;
coincident peer vertices remain independent grips. Mixed object/grip batches
are staged atomically, and Undo/Redo restore the geometry and grip state.

## Kernel and interchange

`TriangleMesh::try_from_face_records` admits stored records with finite points,
distinct raw face indices in range, and at least one face. It allows coincident
or collinear positions. `try_with_edited_vertices` moves raw vertices one to
one while preserving metadata. Newly generated geometry still uses strict
`try_new_faces` validation; `validate_face_geometry` checks that policy
explicitly. This geometric check is stricter than Rhino's `Mesh.IsValid`,
which accepts the captured collinear faces with distinct corner positions.

Exact-location topology omits collapsed sides. Remaining sides retain all
incident uses: the captured mixed Mirror edge has faces `[0,1,1]`, rather than
deduplicating face 1. Area contributions of collapsed triangles are zero.
Closest-point queries project onto the surviving segment or point; an exact
rational fallback handles degeneracy, very thin triangles and intermediate
overflow. Polygon indices and their deterministic display triangulation remain
aligned. Undefined unit normals return an error; viewport shading omits such
facets from neighboring normal averages. Collapsed meshes cannot certify a
solid solely through edge topology.

3DM read/write preserves collapsed records, vertex colors and n-gons. Ordinary
maps can transform already collapsed meshes or heal them. Strict maps of a
healthy source still reject newly collapsed geometry; grip edits use the
record-preserving path. STL export rejects zero-area facets before replacing an
existing destination. Other normal-dependent operations can return explicit
errors on collapsed geometry and have not all been compared with Rhino.
Radial unwelding rejects affected faces with multiple coincident corners
before rebuilding their controls; other faces can retain stored records.

## Evidence and limits

Six fixed recipes were captured using public
[GripUpdate](https://developer.rhino3d.com/api/rhinocommon/rhino.docobjects.tables.objecttable/gripupdate)
and SDK geometry queries in an empty owned Rhino 8.32.26160.13001 document on
private Xvfb. They cover mixed quad/triangle Mirror, triangle line/point,
and quad edge/line/point collapses. Every recipe retains its face records and
colors, and recovers its complete original snapshot after restoring grips.
Kernel tests replay all six recipes and compare records, edge incidences,
polygon normal directions, area and closest points at `1e-12`. The earlier
command fixture's Mirror case now passes both direct and incremental command
paths, including display, picks and Undo/Redo.

The SDK reports Origin for a zero-area mass centroid. Our area properties
retain area zero and explicitly report an undefined centroid. Native archive
interoperability, every edit on invalid meshes, repeated raw indices, arbitrary
nonplanar collapses, snapping and performance equivalence remain unproven.
Independent regressions cover 3DM round trips of the six captured edited
records plus an all-point n-gon, extreme/subnormal closest queries, and preview
and committed scene generation in all four views and all three display modes.
Full Rhino parity remains in progress.

## Reproduce

```sh
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle rhino \
  tools/rhino_oracle/fixtures/mesh_edit_records.json \
  --scheme VibocerosOracleMeshRecords_20261004 \
  --output tools/rhino_oracle/observations/mesh_edit_records.json --timeout 240
cargo test -p viboceros-geometry face_record_tests
cargo test -p viboceros-io collapsed
cargo test -p viboceros grip_transform_replays_native
cargo test -p viboceros collapsed
python3 -m unittest tools.rhino_oracle.test_mesh_edit_records
```

The live helper accepts only the six fixed recipes, one iteration, a private
settings scheme and an empty owned document. The client requires a dedicated
headless display before launching Rhino. [Capture provenance](mesh-edit-records-provenance.json)
