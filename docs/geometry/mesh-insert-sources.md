# Mesh insertion sources and rejected picks

`InsertPoint` accepts a retained quad whose boundary has area even when a flat
corner gives one fixed diagonal triangle zero area. The fallback requires four
distinct positions and a weakly convex projected boundary. Collapsed triangles,
coincident corners and backtracking boundaries still reject the edit. Unchanged
source records survive; newly generated faces keep strict geometric validation.

Native quad triangulation can retain zero-area triangle records. That storage
conversion now preserves the public OpenNURBS result and its vertex channels.
Geometric validation remains available separately, and quadrangulation keeps its
existing strict result checks.

A pick on a disconnected mesh island returns an error without changing the
accepted preview or point stack. A subsequent valid pick continues the path.
Local Undo removes the last accepted point, including after a rejected pick.
The complete batch geometry API rejects an unconnected path atomically.

## Evidence and remaining work

Ten authored Rhino getter workflows cover flat corners, winding, cyclic origins,
a tilted plane, untouched neighbors, disconnected islands, continuation and local
Undo. Command and application replays check coordinates, oriented cells, channels,
metadata, preview isolation and document Undo/Redo at the unchanged `1e-6` bound.
Eighteen separate public API conversion checks compare stored face tables exactly.

Two repeats of 13 older inputs reproduce their original coordinates and stored
cells. A further 26-case capture retains eight incomplete inputs and one explicit
Native tessellation failure. Its remaining 17 cases are meshing diagnostics;
new and old cell/facet/surface gaps remain explicit. A public `Mesh.IsValid`
result on a reconstructed vertex/face table does not prove preservation of the
original region: the failed tessellation also returned a structurally valid mesh.
Public validity/conversion inputs are output-derived calibration data and do not
qualify command input behavior.

The [receipt](mesh-insert-sources-validation.json) distinguishes the ten qualified
workflows from all diagnostics. The rejected alternative tiling rules were not
adopted. Broader warped and repeated-edge layout parity still needs work.

```sh
cargo test -p viboceros-geometry --test mesh_insert_sources
cargo test -p viboceros-command --test mesh_insert_paths
cargo test --bin viboceros flat_corner_sources_and_rejected_island_picks
python3 -m unittest tools.rhino_oracle.test_mesh_insert_sources
```
