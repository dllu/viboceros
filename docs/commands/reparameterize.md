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

`Automatic` uses curve length for curves. For surfaces and faces, it averages
positive isocurve length estimates at eleven evenly spaced parameter values in
each transverse direction, including the boundaries. Collapsed isocurves are
excluded. The length estimates use the existing bounded Cash–Karp integrator
with the public default fractional tolerance of `1e-8`.

This [automatic surface sizing policy](../automatic-surface-domains.md) resolves
the earlier cylinder and sphere range mismatches. The general NURBS
`estimated_size` query continues to report maximum control-polygon lengths.

The saved public-command observations include all six individual faces of a
joined box, an annulus, a reversed rectangular trim, a cylinder and a sphere,
with ordinary ranges, unequal U/V scales and Automatic. An additional 24
Automatic captures cover warped and weighted patches, an unclamped surface,
cones, knot edits, chart edits, scales, weight gauges and document tolerances. Shared source 3dm files
preserve the actual reference topology and parameterization. Both the command
and public `BrepFace.SetDomain` results are retained; its control-polygon size
query differs from the actual command on curved inputs. The reference is
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
