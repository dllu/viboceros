# Mesh, SubD and surface-edge lengths

[Length behavior](../commands/measurements.md) · [Oracle overview](../oracle.md)

The three `component_length_*` families contain 42 actual Rhino Length workflows:
23 preselection controls, seven bare-command follow-ups and 12 viewport-input
controls. Sources include curved grids, creases, extraordinary fans, a closed
box, a surface and mixed curve/edge inputs. Each capture uses private Xvfb and
an isolated Rhino settings scheme. Responses and content-addressed producers
are immutable; compressed responses preserve the original bytes.

Smooth SubD appearance measures the prepared limit-edge NURBS curve. Flat
appearance measures the straight control edge. Mesh edges use topology endpoint
distances. Preselected component references are released after measurement,
including surface edges; preselected whole curves survive. Command-first picks
are cleared on acceptance or cancellation. Bare `_Length` follow-ups distinguish
this cleanup from a trailing macro Enter. Source CRCs remain unchanged and a
seeded Redo/Undo cycle checks both geometry and selection.

Rust constructs the recipes independently and matches all saved selection states
and lengths within native three-decimal print precision. In-memory app replays
cover every input, while real egui events verify mesh and smooth/flat SubD edge
picks in wireframe, shaded and ghosted views without opening windows.
A release Python API replay checks 465 workflows across these cases and the
existing chain, loop, ring and face-loop families.

Separate tests compare 16 prepared SubD edge curves with the public native
`SubDEdge.ToNurbsCurve` output: 65 stations match within `1e-11` model units,
allowing reversed orientation, and lengths match within `1e-6`. Regular curves
use exact one/two-span forms; extraordinary curves use the existing eight-span
fit from exact limit stations. This sampling does not establish a continuous
error bound for every possible net. Source size and subdivision budgets remain
those of the existing SubD conversion kernel.

Topology and limit curves are prepared once per component owner. A 5,000-quad
strip test checks all 15,001 unit edges, unchanged geometry and pending redo.
Nested mesh/SubD SelChain and partial edge measurement remain separate work.
