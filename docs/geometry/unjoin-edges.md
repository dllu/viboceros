# Separate joined B-rep edges

`Brep::try_unjoin_edges(&[edge_index, ...], tolerance)` separates edges shared
by distinct faces and returns all resulting edge-connected components. Indices
are zero-based. Naked edges and seams within one face are ignored. A selection
with no eligible edges returns an empty vector. All indices are validated first;
duplicates are processed once in their first-occurrence order.

The implementation splits endpoint vertices when cutting edges disconnects
their local trim-corner fans. It preserves exact surfaces, UV trims, spatial
curves, domains, tolerances, face orientation, and untouched topology. It never
welds coincident points or refits geometry. Every complete result is validated
before return; the input is immutable.

Components follow source face order. Edges retain source order, followed by
copies in selection order. Vertices follow their first trim use within each
component. Scratch tables hold indices and union/find state; geometry is cloned
when constructing the final components.

The [26 source recipes](../../tools/rhino_oracle/fixtures/brep_unjoin_edges.json)
and [Rhino 8.32 captures](../../tools/rhino_oracle/observations/brep_unjoin_edges.json)
compare complete geometry and topology definitions on identical independently
exported 3dm inputs. Cases cover box vertex fans, split edges, reversed faces,
open face pairs, tube seams and cap boundaries, a sphere seam, a nonmanifold
three-face edge, disconnected compounds, empty selections, and duplicates.
Absolute epsilon is `1e-9`, with relative epsilon zero. These untimed captures
make no performance claim.

Command captures also check split natural boundaries. Surface classification
accepts complete side chains with multiple trims and rejects gaps, overlaps,
reversed fragments, and off-boundary endpoints.

```sh
cargo test -p viboceros-geometry brep::unjoin
cargo test -p viboceros-oracle unjoin_edges
python3 -m tools.rhino_oracle.unjoin_edges_replay tools/rhino_oracle/fixtures/brep_unjoin_edges.json tools/rhino_oracle/observations/brep_unjoin_edges.json
```

See the [UnjoinEdge command](../commands/unjoin-edge.md) for document selection,
attributes, and history. Native mouse modifiers, additional nonmanifold seam
arrangements, and timings remain outside this measured corpus.
