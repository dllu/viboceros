# Reparameterize

Change a curve's parameter range or the U/V ranges of a surface without changing
its spatial shape. Select one curve or surface, then use:

```text
Reparameterize -4 6
Reparameterize -2 8 10 30
Reparameterize Automatic
```

Single-face B-reps, including trimmed surfaces, holes, periodic seams and poles,
retain their trims. For a polysurface, specify its face index. `Surface=id`
provides an explicit editable object reference and does not require selection:

```text
Reparameterize Face=2 -2,8,10,30
Reparameterize Surface=object-id Face=2 Automatic
```

Only the addressed face's support and p-curves change. Spatial edges and
vertices, neighboring face charts, orientation, object identity, attributes,
groups and user text remain intact. The edit is one undo step; failures and
unchanged ranges preserve the redo branch. Changes survive 3dm export/import.
The command uses the shared validated affine
[face chart mapper](../fillet-parent-charts.md), including its refusal of charts
that lose excessive coordinate precision.

`Automatic` retains the existing size policy: curve length for curves and the
maximum control-polygon length in each direction for NURBS surfaces and faces.
This corresponds to the open-source NURBS `GetSurfaceSize` estimate. It differs
from Rhino's actual command on the recorded curved examples: for a radius-two
cylinder Rhino chooses a U range near `0..12.56637`, while this estimate gives
`0..16`. The sphere example also differs. These two command-domain mismatches
remain explicit regression controls; matching Rhino's curved Automatic policy
is unfinished. Explicit ranges match all recorded command cases at `1e-8`,
and normalized surface/trim image checks separately exercise all 30 recipes.

The saved public-command observations include all six individual faces of a
joined box, an annulus, a reversed rectangular trim, a cylinder and a sphere,
with ordinary ranges, unequal U/V scales and Automatic. Shared source 3dm files
preserve the actual reference topology and parameterization. Both the command
and public `BrepFace.SetDomain` results are retained; the latter's size query
also differs from the actual command on some curved inputs. The reference is
SDK-only; replay runs through the Viboceros command tests, rather than a new
Rust oracle protocol operation. Viewport face picking and automatic copying
from block instances remain outside this command's current workflow.

```sh
cargo test --release -p viboceros-command --lib reparameterize
cargo test --release -p viboceros-command --test reparameterize_faces
python3 -m unittest tools.rhino_oracle.test_reparameterize_face_probe
```

See the [Rhino command documentation](https://docs.mcneel.com/rhino/8mac/help/en-us/commands/reparameterize.htm),
[public face-domain API](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/M_Rhino_Geometry_BrepFace_SetDomain.htm),
and [validation record](../reparameterize-faces-validation.json).
