# Separate joined surface edges

[Command reference](README.md)

Run `UnjoinEdge` and click joined B-rep edges, or select them with rectangles.
Enter applies the batch; Escape or `Cancel` discards it. Overlapping candidates
offer numbered choices with highlighting. View and CPlane commands preserve the
selection prompt. A new geometry command cancels the pending batch.

Ctrl+Shift (Command+Shift on macOS) selects surface/B-rep components before
starting the command. Eligible preselected edges apply immediately and finish
the command. A subsequent `Cancel` does not undo that completed edit. Whole
objects, faces, naked edges, and seams within a single face do not qualify.
Component selection clears on completion and is independent of Undo/Redo.

Typed invocation accepts multiple objects and comma-separated edge indices:

```text
UnjoinEdge object-id 0,1,2,3
UnjoinEdge first-object-id 0,3 second-object-id 2
```

Edges shared by different faces are separated without refitting. Each source
produces its resulting edge-connected components. Its first component keeps
the source object identity; additional components inherit the source's name,
layer, object attributes, groups, and geometry user text. Outputs are unselected.
One external Undo/Redo restores the complete batch, including object ordering.

`UnjoinEdgeSelection::prepare` validates and stages a complete batch without
editing the document. `commit` rejects changed geometry, document tolerance,
visibility, or locking before applying any source. No transaction stays open
while the viewport waits for more picks. Empty batches preserve Redo.

`prepare_preselected` uses component table order; `prepare` gathers mouse or
typed references in input order. The measured command policy reverses the
preselected edge table and rotates each source's gathered references left once
for batches of at most eight eligible edges. Larger batches keep their gathered
order. This affects copied-edge indices; the geometry API preserves its supplied
order independently. Source indices are checked and duplicates, naked edges,
and seams are filtered before this policy is applied.

The 48 saved native command captures exercise preselection, mouse batches,
reversed selection order, seams, wrong component kinds, whole-object selection,
multiple sources, cancellation, metadata, and genuine external Undo/Redo.
Full geometry definitions, source identities, ordering, and transient component
state are compared. The public geometry API has a separate
[26-case corpus](../geometry/unjoin-edges.md).
Postselection removal with modifier keys and native rectangle captures remain
pending; current postselection clicks and rectangles add eligible edges.
Single-source preselection ordering is measured through twelve edges, with
two- and three-edge mouse orders and two-source batches. Larger mixed batches
and reference platforms beyond this Rhino 8.32 Wine host remain unmeasured.

```sh
cargo test -p viboceros-command unjoin_edge
cargo test -p viboceros app::tests::unjoin_edge
cargo test -p viboceros-oracle unjoin_edge_command
python3 -m tools.rhino_oracle.unjoin_edges_replay tools/rhino_oracle/fixtures/unjoin_edge_command.json tools/rhino_oracle/observations/unjoin_edge_command.json
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.unjoin_edges_capture tools/rhino_oracle/fixtures/unjoin_edge_command.json --timeout 300
```

Saved replay uses no Rhino GUI. All live captures use a dedicated private Xvfb
display and target only windows and processes owned by that job.
