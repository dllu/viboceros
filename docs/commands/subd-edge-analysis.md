# SubD diagnostic edges

[`ShowEdges`](show-edges.md) accepts SubDs, displays their diagnostic edges and
clears their selection. All mode includes naked, hard-crease and soft-crease
edges; smooth interior edges remain hidden. Naked mode includes boundary edges.
`ZoomNaked` supports command-first picks, repeated navigation and marking, Enter,
Escape and one-step Undo/Redo. Accepted marks survive Escape. Preselection stays
selected through Zoom and its history; command-first picks remain deselected.

Smooth display uses limit curves and limit endpoints; flat display uses control
segments and control endpoints. Appearance changes refresh the shared edge cache
without editing model geometry or history. Hard creases with crease/corner ends
use direct cubic Bezier coefficients, avoiding dense subdivision. Other edges
use the existing bounded limit-curve fitter. All four views and wireframe,
shaded and ghosted modes share overlays and endpoint markers.

## Evidence and limits

Three immutable Rhino 8 families cover 54 supported workflows:

| Family | Cases | Evidence |
| --- | ---: | --- |
| `subd_edge_analysis_discovery` | 16 | Named ShowEdges, native counts and radio controls, pre/postselection, both appearances |
| `subd_edge_analysis_controls` | 32 | Named ZoomNaked, ordered duplicate endpoints, navigation, Enter/Escape, Undo/Redo |
| `subd_edge_analysis_capabilities` | 6 | Hard/soft-crease inputs and actual enabled states of native panel controls |

Public native edge curves supply 33 parameter stations per displayed edge.
Portable replays compare those images within `1e-9`, allowing either direction
for an undirected source edge. Model states use `1e-12`, exact topology and IDs,
selection, point counts and point creation order. Polygon scenes are independently
constructed; soft-crease scenes use existing public 3DM fixtures. Marking uses
shared operands so component direction has the same meaning. Their manifest links each input to its frozen capture bytes.
An additional 4,800-face curved-grid test exercises the direct crease path, cache
reuse, appearance refresh and preservation of pending Redo.

Native Edge Analysis disables its Zoom button in All mode. Four message-driven
and four mouse-driven panel diagnostics retain their failed input traces and
unchanged models; they are not successful marking evidence. The current capture
driver rejects disabled controls. Viboceros' existing `ShowEdges Zoom ...` and
`ShowEdges Mark` console bindings remain extensions, including All-mode marking.

Six negative/control probes also establish that native ShowDir leaves open and
closed SubDs unanalyzed while its line and mesh controls work. These admission
records do not qualify SubD persistent direction display.

Curves near darts and soft creases retain the existing fitter's subdivision
budget and approximation limits. Station comparisons do not establish a
continuous-error bound or identical native pixels. Captures cover valid manifold
SubDs; non-manifold SubD file repair remains separate work.
[Mixed-source workflows](mixed-edge-analysis.md) qualify marking order, source
membership and enabled Naked-mode panel Zoom. Analysis state is transient, and restored deleted sources must be added again.

```sh
cargo test -p viboceros-command --lib edge_analysis
cargo test -p viboceros-oracle --test subd_edge_analysis
cargo test -p viboceros --bin viboceros subd_edge_analysis
cargo test -p viboceros --bin viboceros viewport::edge_picking
python3 -m unittest tools.rhino_oracle.test_subd_edge_analysis
python3 -m tools.rhino_oracle.subd_edge_analysis_replay
```

Fixtures, gzip observations and hashed producer snapshots live under
`tools/rhino_oracle/`. Native runs use private Xvfb sessions. The release Python
replay needs no Rhino or desktop window.

Reference: [Rhino 8 ShowEdges](https://docs.mcneel.com/rhino/8/help/en-us/commands/showedges.htm).
