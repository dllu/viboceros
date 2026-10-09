# Native edge marking and command navigation

`ZoomNaked` and `ZoomNonManifold` now retain two point objects per marked edge,
including coincident endpoints and shared corners. Navigation and repeated
Mark options execute in input order, and one Undo reverses every mark made
during that command. New Zoom commands reset the current edge. Pending camera
fits survive color edits and marking until the application acknowledges them.

The [50 recipes](../tools/rhino_oracle/fixtures/edge_mark_workflow.json) drive
actual licensed Rhino command macros under private Xvfb. They cover bilinear
surfaces, a rectangular-hole B-rep, welded triangle pairs, a non-manifold edge,
and mixed surface/mesh selection in both source orders. Options include current
and All focus, repeated Next, wraparound, Previous, Current after Next or All,
repeated Mark, Mark/Next/Mark, and eight Undo/Redo pairs. Every model point,
input count, command string and success flag is retained in the
[observations](../tools/rhino_oracle/observations/edge_mark_workflow.json).

All 50 workflows and 654 numeric fields match exactly. The comparator uses
1e-9 absolute plus 1e-12 relative epsilon and excludes no fields. Workflow
timings are not measured; zero elapsed values do not establish performance
parity.

Point records use creation order: Rhino's public `RuntimeSerialNumber` orders
the inserted point objects, while Viboceros retains insertion order. Native
object-table enumeration returns these points in reverse order; the preliminary
[research records](edge-mark-research-source/viboceros-edge-mark-native.json)
retain that raw enumeration and serial numbers. No coordinate sorting, point
deduplication or geometry normalization is applied to the replay.

Naked/non-manifold All marking processes B-reps before meshes, even when the
mesh was selected first. Current navigation retains the selected source order.
Within the captured objects, edge order and direction match their topology
records. An initial reversed-trim hypothesis was rejected by the replay: the
surface's U direction ran along world Y, so interpreting it as world X gave
the wrong edge correspondence. The final implementation keeps the original
edge topology order and directions.

The initial history probe ran inside `RunPythonScript`. Undo reported success
but left marked points unchanged; those [diagnostic observations](edge-mark-research-source/nested-workflow-native.json)
remain available. The final worker schedules command workflows at Rhino idle,
after the Python command returns. Undo then removes all marked points, retains
the source objects, and Redo restores the marks. The exact shipped Python
helpers are preserved in [capture sources](edge-mark-capture-source).

```sh
WINEDLLOVERRIDES='sspicli,secur32,schannel=b' \
  tools/rhino_oracle/run_headless.sh rhino \
  tools/rhino_oracle/fixtures/edge_mark_workflow.json \
  --scheme VibocerosOracleEdgeMarkWorkflow20261009 --timeout 240 \
  --output /tmp/viboceros-edge-mark-workflow-native-idle.json
cargo test -p viboceros-oracle --release --test edge_analysis
```

The Python `edge_analysis` operation accepts an optional `workflow` object:
`{"command":"ZoomNaked","actions":["Mark","Next","Mark"],"undo_redo":true}`.
Actions are bounded to 32 and restricted to All, Current, Next, Previous and
Mark. Live workflows require a private settings scheme and one iteration.

[Comparison](edge-mark-comparison.json) and [provenance](edge-mark-provenance.json)
record the result, epsilon and source hashes. Native Edge Analysis dialog
controls, exact camera framing, ShowEdges All-mode ordering, broader curved and
multi-face B-reps, and display/session lifetime remain separate qualification
work. These captures do not establish complete Rhino compatibility.
