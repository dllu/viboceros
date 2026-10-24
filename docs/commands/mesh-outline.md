# MeshOutline

Select meshes, surfaces or polysurfaces and run `MeshOutline`. With no eligible
preselection, pick objects and press Enter; Esc cancels. Overlapping objects
contribute to one combined outline, including visible holes and disconnected
regions. Sources remain unchanged. Output polylines use the current layer and
default attributes, and one Undo removes all output curves.

The active viewport supplies the projection at acceptance. Parallel views aligned
with the construction plane project there; other views use normalized clipping
depth zero. Camera changes during selection are respected. Eligible sources are
deselected and outputs selected; ignored preselected peers remain selected until
selection is cleared or history is replayed. Redo restores curves unselected.

Surfaces use the same display mesh routines and sampling policy as Viboceros's
viewport. Curved surface tessellation does **not** yet match Rhino's render mesh:
the captured sphere is an explicit geometry gap. The recorded planar surface and
box workflows match. Projecting Rhino's saved sphere
render mesh also matches, separating this gap from outline projection.
Small-scale perspective projection of warped quads has a separate gap, recorded
in two workflows. Native SDK controls reproduce that scale-dependent change.
Flattening quads before triangulation would break the larger-scale controls, so
the implementation retains the existing spatial split pending further evidence.

The [command qualification](../mesh-outline-command-qualification.json) compares
36 accepted commands and 39 getter workflows, including cancellation, metadata,
selection and Undo/Redo. One curved surface and two perspective projection gaps
remain in each replay group; all lifecycle and metadata checks pass.
An earlier 32-scene capture recorded only the initial viewport; the later capture
also records getter-time camera and clipping changes. Getter replays apply that
captured viewport before acceptance. Geometry is compared with `1e-7` model-unit
tolerance; contour order, start vertex and redundant collinear points may differ.

The geometry API, limits, immutable SDK captures and reproduction instructions
are described in [mesh outline geometry](../mesh-outline-geometry.md).

```sh
cargo test -p viboceros-command --test mesh_outline_command_native -- --nocapture --test-threads=1 > /tmp/outline-command.log 2>&1
cargo test --bin viboceros mesh_outline_getters -- --nocapture --test-threads=1 > /tmp/outline-getter.log 2>&1
python3 -m tools.rhino_oracle.report_mesh_outline_command /tmp/outline-command.log /tmp/outline-getter.log
python3 -m unittest tools.rhino_oracle.test_mesh_outline_command
```
