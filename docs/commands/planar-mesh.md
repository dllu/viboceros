# PlanarMesh

`PlanarMesh` creates editable meshes from planar curves. Select the inputs first
to run immediately, or start the command, pick curves and press Enter. Escape
cancels selection without consuming Redo.

```text
PlanarMesh
PlanarMesh Sources=<id>,<id>
```

`Sources` is a scripting extension for explicit object IDs. Connected open
segments can form a closed boundary. Inner loops cut holes, further nested
loops create islands, and boundaries on different planes create separate
regions. Overlapping or touching boundaries remain independent input regions.
Nonplanar and unclosed inputs are ignored when usable boundaries remain;
otherwise the command fails without changing geometry or history. Failed
preselected inputs remain selected; inputs picked during the command are released.

The original curves and their attributes remain unchanged. Successful output
is unnamed and unselected on the current layer. Input selection clears on
success. All outputs share one Undo operation; Undo leaves the sources
unselected. Invalid explicit IDs, duplicate IDs and unsupported arguments fail
before document edits.

The native build reuses `usd-brep` curve admission and planar-face construction.
Existing portable construction handles boundary contacts and native producer
rejection after admission. Both builds use the shared curve joiner, constrained
patch triangulation and rectangular mesh grid. Rectangles produce four-by-four
quad grids; other regions sample each trim span sixteen times. Input is bounded
to 128 profiles and 100,000 joined control points. Each planar B-rep is bounded
to 100,000 sampled boundary stations, with shared constrained-recovery limits.

The [qualification report](../planar-mesh-qualification.json) preserves 21 native
command captures, three additional native postselection controls, and
native/portable command and pre/postselection UI replays.
All surface comparisons stay within `1e-6`, including circles, an ellipse,
concave regions, holes, islands, joined lines and rejection cases. Eleven cases
still differ in vertex, face or triangle counts; these topology gaps remain
unqualified. General adaptive meshing, native face ordering and comparative
speed need further qualification. The retained single-boundary SDK results use
different settings and are separate from the command targets.

Reference: [McNeel PlanarMesh command](https://docs.mcneel.com/rhino/8/help/en-us/commands/planarmesh.htm).
