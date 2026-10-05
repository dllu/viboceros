# Chained polyhedral Booleans

[Architecture](architecture.md) · [Convex foundation](convex-booleans.md) · [Oracle](oracle.md)

`boolean_polyhedral_breps(first, second, operation, tolerance)` computes a
mathematical union, intersection, or difference of two closed polyhedral B-reps.
It returns separate material components, each retaining its cavity shells and
original operand/face ownership. `Brep::try_boolean_polyhedral` returns the same
result combined into one B-rep, or `None` when the region is empty. Inputs remain
unchanged. The existing convex APIs and interactive command policies are separate.
The result represents solid material: isolated zero-volume face, edge or point
intersection patches are discarded, and retained boundaries are closed.

## Input certificates

Concave faces, face holes, multiple disconnected shells, cavities, and nested
material islands are supported. Material follows odd/even containment of shells,
independently of their input orientation. Supporting surfaces must have exactly
affine, clamped degree-one 2×2 control nets with uniform weights. Model vertices
must lie exactly on their supporting planes.

Edges and UV trims must have clamped, continuous, same-sign rational control
hulls contained in their endpoint segments. This accepts the straight splines
produced by `MergeAllEdges`; 3D edge endpoints must equal their stored model
vertices exactly. The exact endpoint discrepancy between each lifted
UV trim and its model edge must be within the caller's absolute tolerance. Since
both entire images are straight segments, their endpoint bound certifies their
geometric correspondence over the whole segment. Rounded UV values such as
`1/3` can therefore be accepted without moving model vertices or weakening the
plane certificate.

Exact predicates check simple loop boundaries, natural outer/hole winding,
strictly enclosed disjoint holes, and face intersections. Different input faces
may meet only at their shared topological edges or vertices. Overlapping input
shells and unconnected geometric contacts are rejected. Curved and open inputs,
nonaffine or noncoplanar roundoff, singular trims, and unresolved rounded output
topology return errors. An arbitrary earlier Boolean result may still fail an
exact plane certificate when Cartesian rounding affected its plane incidence.

## Construction

Supporting lines partition a concave or holed face into convex arrangement cells.
Exact winding tests select cells in the original face region. Supporting planes
of both operands then split their boundary patches. Coplanar patches are also
split at their bounded edges so competing source faces construct identical cells.

Each cell has an exact interior witness and two exact normal offsets whose step
is bounded by the next supporting plane. Retried exact ray parity classifies
material on each side. A cell bounds the result precisely when the requested set
expression differs between those sides; its orientation points away from material.
Duplicate coplanar cells belong to the earliest operand and face. Shared edge
subdivisions resolve T-junctions before Cartesian and UV coordinates are rounded
once and ordinary B-rep validation runs. Exact shell volumes and containment
attach cavities to their enclosing material components.

Limits are 256 faces per input, 4,096 convex fragments per extraction,
arrangement or output, two million work units, and 8,192 bits per checked rational.
Exhaustion is an explicit error. Coplanar fragments remain separate; the existing
face merging API can merge them by original source or across source faces.

## Verification

Focused tests compare mathematical volumes for chained concave, holed and cavity
solids, disjoint shells, nested islands, reversed inputs, tetrahedra, equality and
contacts. They check original supporting surfaces, input purity, unsupported
geometry, intersecting input shells, and work limits.

Thirty native public SDK captures run at idle in an empty owned document on private
Xvfb, with private settings. Raw observations retain source geometry, null versus
empty responses, mass properties, bounds, polygon loops, vertices, and nine
stations per edge. Replay allows different face partitions and compares physical
geometry with finite bidirectional boundary witnesses. These witnesses do not
prove continuous error bounds, general curved Boolean parity, interactive command
parity, or performance against Rhino. The 24 matching calls use epsilon `1e-10`
for volume, centroid and tight bounds, `1e-9` for area, and `1e-7` for boundary
witnesses. Two native unions return a cavity as a separate inward B-rep; their
signed aggregate volume, centroid, area, and physical boundary match the kernel's
single material component with its cavity attached.

Six observed differences remain explicit:

| Recipe / operation | Kernel | Captured public SDK |
| --- | --- | --- |
| Cavity / intersection | Volume 2 | Separate solids, total volume 6 |
| Nested island / intersection | Volume 7.125 | Separate solids, total volume 34.125 |
| Touching hole corners / intersection | Reject singular boundary | Valid, open B-rep |
| Reversed hole / union | Volume 26, area 70 | Inward solid, volume −22, area 66 |
| Reversed hole / intersection | Volume 2, area 10 | Two solids, total volume 2, area 14 |
| Reversed hole / difference | Volume 22, area 66 | Inward solid, volume −26, area 70 |

These are SDK observations for the closed recipes, not interactive command
policies. Raw outcomes are retained in
[`observations/polyhedral_boolean.json`](../tools/rhino_oracle/observations/polyhedral_boolean.json);
see [capture provenance](polyhedral-boolean-provenance.json).

The Python oracle operation `polyhedral_boolean` accepts the closed `case` and
`operation` fields from the fixture. Its Rust adapter independently builds the
sources, computes the result, and returns matching physical geometry records.
Unrepresentable boundaries have an explicit `kernel_error` record.
`tools.rhino_oracle.polyhedral_boolean.compare_polyhedral_responses` compares
physical inputs and signed aggregate output regions while permitting different
face partitions. It reports semantic differences and performs no live capture.
Signed native cavity volumes are retained; source orientation is allowed to
differ when comparing the same physical input region.

```sh
cargo test --release -p viboceros-geometry brep::boolean::polyhedral -- --nocapture
python3 -m unittest tools.rhino_oracle.test_polyhedral_boolean
python3 -m tools.rhino_oracle viboceros tools/rhino_oracle/fixtures/polyhedral_boolean.json --output /tmp/polyhedral-viboceros.json

# Live captures always use an owned private display and settings scheme.
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle rhino tools/rhino_oracle/fixtures/polyhedral_boolean.json --scheme VibocerosOraclePolyhedralExample --timeout 360 --output /tmp/polyhedral-rhino.json
```

Compare a generated local response with the saved capture without launching Rhino:

```python
import json
from pathlib import Path
from tools.rhino_oracle.polyhedral_boolean import compare_polyhedral_responses

local = json.loads(Path('/tmp/polyhedral-viboceros.json').read_text())
native = json.loads(Path('tools/rhino_oracle/observations/polyhedral_boolean.json').read_text())
differences = [row for row in compare_polyhedral_responses(local, native) if not row['passed']]
print(differences)
```
