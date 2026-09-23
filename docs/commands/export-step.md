# ExportStep native mode

```text
ExportStep Native=Yes "editable part.step"
```

The default `ExportStep` writes a faceted shell from document geometry.
`Native=Yes` writes document B-reps as editable STEP `ADVANCED_FACE` and
`EDGE_CURVE` records. Planar straight-edged models use plane and line entities;
other nonsingular B-reps use rational B-splines where curves or surfaces need
them, with explicit face-local p-curves on curved faces. Both paths retain
shared edge topology and face orientation without mesh tessellation.
The `ExportStp` alias accepts the same option. Quote a filename beginning with
`Native=Yes` to use that text as a path.

Native export requires every document object to be a B-rep. Singular trims,
closed or repeated seam edges, and NURBS weight configurations with poles are
errors, with no mesh fallback. STEP coordinates and the declared distance
accuracy are converted from document
units to millimetres. Export reads the document without changing objects or
undo history. A failed export leaves an existing destination file unchanged.

Certified convex planar polyhedra, including tetrahedra and sheared boxes,
export as STEP solids. Strictly contained, inward-oriented convex cavities
remain in the same `BREP_WITH_VOIDS` shape even if a cavity shell comes first.
Certification uses exact coordinate predicates and requires straight edges,
convex face polygons, and a consistent closed boundary. Cavities that touch or
overlap another shell, or lie outside the outer shell, do not receive void
semantics. Other edge-disconnected shells become separate STEP surface models.
General compound B-rep identity, names, layers, groups, and materials are not
yet serialized by this mode. Supported curved exports can be read back with
`ImportStep Native=Yes` as editable B-reps. Convex solid, cavity, and
polygon hole round trips check editable topology, orientation, area or volume,
and units.
Mixed-object and unsupported-geometry tests check explicit failure and atomic
destination replacement.

See [file-format limits](../file-formats.md) and [native STEP import](import-step.md).
