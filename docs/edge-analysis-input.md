# Edge-analysis command input and session lifetime

Bare `ZoomNaked` and `ZoomNonManifold` now collect eligible objects and continue
with a command-line option prompt. Type All, Current, Next, Previous or Mark;
options may repeat. Enter ends the prompt. Escape also ends it, retaining
accepted marks. Marks created across prompt answers share one Undo step, and
Redo restores them. Invalid answers leave the prompt and model intact; starting
another modeling command ends the current Zoom input.

Complete lines such as `ZoomNaked All Mark` and
`ShowEdges Show=All Color=20,40,60` retain their arguments when object selection
is needed. ShowEdges Add/Remove also support command-first selection. Eligible
sources remain NURBS surfaces, B-reps and meshes.

The persistent ShowEdges panel and temporary Zoom input use independent
registry sessions. A Zoom command does not open the Edge Analysis panel and
does not replace an existing panel's sources, color or mode. Both overlays may
appear together; the shared display cache retains two sampled edge collections
so alternating viewport draws reuse their buffers. Completed one-shot Zoom
commands clear temporary display after the pending camera fit is applied.

Five [licensed Rhino observations](edge-input-capture-source/native.json),
captured under private Xvfb, establish the panel lifetime and Escape behavior.
Zoom completion leaves no Edge Analysis window; ShowEdges opens one; running
Zoom afterward leaves that window open. Canceled Zoom returns false but retains
its accepted points, which Undo removes. The capture uses public Windows window
titles and RhinoCommon geometry, with exact [helper bytes](edge-input-capture-source/helper.py).

The command API accepts final Enter/Cancel tokens for Zoom commands. Cancellation
returns `OperationDeclined` and commits accepted marks; malformed arguments
continue to roll back. The oracle workflow now accepts
`"finish":"Cancel"`. Sixteen [cancellation recipes](../tools/rhino_oracle/fixtures/edge_zoom_cancel.json)
and [raw observations](../tools/rhino_oracle/observations/edge_zoom_cancel.json)
cover empty cancellation, one mark, repeated marks and Next/Mark on surfaces,
trimmed holes, welded meshes and non-manifold edges, with Undo/Redo where marks
exist. The existing 10 topology and 50 navigation/marking records remain part
of the replay suite.

```sh
WINEDLLOVERRIDES='sspicli,secur32,schannel=b' \
  tools/rhino_oracle/run_headless.sh rhino \
  tools/rhino_oracle/fixtures/edge_zoom_cancel.json \
  --scheme VibocerosOracleEdgeCancel20261009 --timeout 240 \
  --output /tmp/viboceros-edge-zoom-cancel-native.json
cargo test -p viboceros-oracle --release --test edge_analysis
```

[Provenance](edge-input-provenance.json) records the final tests and exact source
hashes. Native panel widgets, highlight styling during input, precise camera
framing, selection-order variants and broader curved/multi-face geometry remain
qualification work. README remains concise; this page documents the detailed
input behavior.
