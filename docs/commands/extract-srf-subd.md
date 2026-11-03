# Extract SubD faces

[ExtractSrf](extract-srf.md) also extracts editable SubD regions. Select faces
with Ctrl/Command+Shift before the command, or click and window-select them while
it runs. Ctrl/Command removes picks; Enter commits and Escape cancels.

```text
ExtractSrf Faces=0,2 Copy=Yes SubDAddCorners=No
ExtractSrf Face=0 Object=<selected-object-id> SubDAddCorners=Yes
```

Selected faces sharing control edges form one output. Regions touching only at
vertices separate. Removing faces also separates disconnected remainders; the
first remainder keeps the original object ID. Each region keeps increasing
source face order. Sources are processed in reverse document order, with outputs
before their renewed remainders. Selecting every face succeeds unchanged and
creates no modeling Undo entry.

`SubDAddCorners` starts at No and is remembered outside model history, including
after interactive cancellation. Yes creases one-face corners on extracted
regions and corresponding ordinary boundary corners on the remainder. Unrelated
corners, creases and retained interior soft sharpness survive. The source stays
unchanged with Copy=Yes. All geometry is staged and validated before editing;
stale sources, attributes, groups, order or tolerance reject the operation.

In the measured Rhino 8.32 build, SubD outputs retain source groups and layers,
including with `OutputLayer=Current`. Output selection does not expand those
groups. This differs from NURBS/B-rep extraction and from the generic group note
in [McNeel's help](https://docs.mcneel.com/rhino/8/help/en-us/commands/extractsrf.htm).
Completion selects extracted regions; remainders stay unselected. One Undo/Redo
step covers all edits. Native script return separately clears retained component
picks; application comparisons use that phase.

The frozen command families cover 54 workflows: bent grids, triangle/quad cages,
closed cages, existing creases, adjacent/disconnected/vertex-only regions,
multiple sources, all-face no-ops, both Copy modes and corner styles. Four further
commands qualify corner-option memory. Public native 3DM source archives preserve
raw edge orientation independently of mesh conversion between SDK versions.
Replays compare source/output controls, raw topology, tags and vertex limit
points at `1e-10`, plus IDs, order, attributes, groups, selection and Undo/Redo.
Application tests exercise both preselected and command-first component inputs;
viewport tests cover the face filter in wireframe, shaded and ghosted modes.

Subsets retain native records without mesh conversion. Topology grouping uses
edge incidence. Archive copies are bounded to a combined 16 million source components and
64 MiB of staged archives; source face corners are limited to 400,000. Ambiguous
parallel source edges are rejected. Typed point selection uses the evaluated
display facets as a picking approximation. Arbitrary nonmanifold cages, symmetry,
packing, texture/material channels and native physical SubD mouse gestures need
further qualification. Limit stations do not certify continuous surface error.

```sh
cargo test -p viboceros-geometry --lib subd_extract
cargo test -p viboceros-command --lib extract_surface
cargo test -p viboceros-oracle --test subd_extract
cargo test -p viboceros --bin viboceros subd_extract
python3 -m unittest tools.rhino_oracle.test_subd_extract
```
