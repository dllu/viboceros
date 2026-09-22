# ExportStep native planar mode

```text
ExportStep Native=Yes "editable part.step"
```

The default `ExportStep` writes a faceted shell from document geometry.
`Native=Yes` writes document B-reps with planar faces and endpoint-exact straight
edges as editable STEP `ADVANCED_FACE` and `EDGE_CURVE` records. It retains shared
edge topology, face orientation, and polygon holes without mesh tessellation.
The `ExportStp` alias accepts the same option. Quote a filename beginning with
`Native=Yes` to use that text as a path.

Native export requires every document object to be a B-rep. Curved edges,
singular trims, and nonplanar faces are errors, with no mesh fallback. STEP
coordinates and the declared distance accuracy are converted from document
units to millimetres. Export reads the document without changing objects or
undo history. A failed export leaves an existing destination file unchanged.

Each edge-disconnected shell becomes a separate STEP shell model. Compound
B-rep object identity, names, layers, groups, and materials are not yet
serialized by this mode. The supported planar geometry can be read back with
`ImportStep Native=Yes`; box and polygon-hole round trips check editable face
topology, orientation, area or volume, and physical units. Curved-input and
mixed-object tests check explicit failure and atomic destination replacement.

See [file-format limits](../file-formats.md) and [native STEP import](import-step.md).
