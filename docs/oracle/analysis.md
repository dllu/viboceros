# Edge analysis and projection comparisons

[Oracle overview](../oracle.md) · [Evidence index](README.md)

## Edge-analysis topology

The `edge_analysis` operation accepts `sources` containing surfaces, trimmed
B-reps, native boxes and meshes. It compares the actual Viboceros ShowEdges
session with public RhinoCommon edge topology, retaining ordered edge indices,
classification flags, parameter domains and seventeen curve samples. It uses
owned geometry without adding Rhino document objects or opening the native
analysis dialog. Ten [native recipes](../edge-analysis-native.md) match all raw
fields; dialog controls and camera behavior remain separate work.

```python
from tools.rhino_oracle.client import OracleClient

request = {"protocol_version": 1, "iterations": 1, "operations": [{
    "op": "edge_analysis", "id": "box", "sources": [{
        "type": "box_brep", "min": [0, 0, 0], "max": [1, 2, 3]
    }]
}]}
native = OracleClient(settings_scheme="VibocerosOracleEdgesExample").run_rhino(request)
```

On Linux the client launches this probe under private Xvfb. See the qualification
record for the successful process-local Wine DLL setting and strict replay.

An optional `workflow` adds actual ZoomNaked/ZoomNonManifold command actions,
for example `{"command":"ZoomNaked","actions":["Mark","Next","Mark"],"undo_redo":true}`.
The [50 command workflows](../edge-mark.md) run at Rhino idle so their Undo records
are independent of RunPythonScript. Point arrays preserve creation order using
public runtime serial numbers; duplicate endpoints remain distinct objects.

Workflow `finish` may be `Enter` (default) or `Cancel`. The
[16 cancellation recipes](../edge-analysis-input.md) retain native false command
results together with accepted points and Undo/Redo state. These are completed
native observations; a canceled command is distinct from a failed worker or
process launch.

`project_curve_surface` accepts an independent NURBS `curve`, NURBS `surface`
and a finite nonzero three-coordinate `direction`. The local adapter executes
Project on a fresh document; Rhino invokes public `Curve.ProjectToBrep` against
an owned natural face. Both report output curves' lengths, closure and 33
samples at equal arc-length intervals, oriented by lexicographic endpoints and
sorted by their first sample. This removes open-curve direction and parameter
chart differences from the six supplied recipes. It does not measure command
metadata parity or establish a continuous correspondence between engines.
See [Project qualification](../commands/project.md) and
`tools/rhino_oracle/fixtures/project_curve_surface.json`. No fresh Rhino
response is retained: the 2026-10-14 isolated attempt timed out during startup.

`project_curve_mesh` accepts `curve`, `vertices`, polygon `faces` and `direction`;
`project_point_mesh` accepts `point` with the same target fields. Local probes
execute Project in a fresh document. SDK probes own temporary mesh/curve inputs
and dispose all projection results. Curve records use the existing 33
arc-length samples, length and closure; point records are sorted lexicographically.
The thirteen `project_mesh.json` recipes include shared edges, misses, rational
curves, two sheets and warped quads. Native/portable agreement is a local check,
not proof of Rhino parity; the fresh Rhino startup attempt timed out.
