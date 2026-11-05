# Mixed edge analysis

`ShowEdges` clears analyzed input selection across B-reps, meshes and SubDs.
Unsupported selected peers in the same group stay selected. `ZoomNaked` retains
preselection, while command-first picks are released. Selection cleanup changes
neither source geometry nor history and leaves pending Redo available.

All-focus marking processes curved sources first, preserving their source order,
then polygonal sources in source order. Smooth SubDs belong to the curved batch;
flat SubD control edges belong to the polygonal batch with meshes. Current-edge
navigation preserves the full source order across types. Duplicate endpoints
remain separate point objects, in native creation order.

The Edge Analysis panel's Add and Remove controls retain preselection. Its Zoom
button starts live Zoom input using analyzed sources, even if current selection
contains a removed source. It reverses analyzed source order before forming the curved/polygonal batches,
starts in All focus, releases supported selection
and leaves persistent analysis intact. All/Current/Next/Previous/Mark work until
Enter or Escape; accepted marks share one Undo step. The button is disabled in
All display mode. Existing `ShowEdges Zoom ...` and `ShowEdges Mark` console
bindings remain extensions.

## Native evidence

Private Xvfb captures from licensed Rhino 8 cover 86 workflows:

| Family | Cases | Coverage |
| --- | ---: | --- |
| `edge_analysis_mixed` | 72 | Six source permutations, pre/postselection, smooth/flat, All marking and cross-source navigation, grouped inputs with an unsupported point |
| `edge_analysis_mixed_membership` | 6 | Actual native Add Objects, Remove Objects, Naked radio and enabled Zoom button for each permutation |
| `edge_analysis_mixed_panels` | 8 | Panel Zoom without membership edits and with sequential additions; distinguishes panel source reversal from insertion order |

Seven shared input archives retain native component ordinals. Their manifest
links exact bytes to the first native record. Immutable observations retain
named-command results, event states, source geometry, attributes, selection,
ordered points, history and panel input traces. Hashed snapshots preserve the
producer versions used for each capture.

Command, app and release Python replays compare before/after/Undo/Redo plus all
membership stages, with `1e-12` coordinate tolerance, exact topology and point
order. App tests use object picking and separate live option answers, preserve
source geometry storage and verify the pending-Redo witness. Membership tests
mark the remaining two sources, producing 16 points; removed selected sources
produce none. Panel-only tests mark all three sources, producing 24 points.

```sh
cargo test -p viboceros-command --lib edge_analysis
cargo test -p viboceros-oracle --test edge_analysis_mixed
cargo test -p viboceros --bin viboceros edge_analysis
python3 -m unittest tools.rhino_oracle.test_edge_analysis_mixed
python3 -m tools.rhino_oracle.edge_analysis_mixed_replay
```

Precise camera framing, native highlight pixels and more complex topology still
need qualification. Analysis state is transient; restored deleted sources must
be added again. See [SubD diagnostic edges](subd-edge-analysis.md) for limit-curve
accuracy and the disabled-All native control evidence.
