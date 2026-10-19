# Reparameterize

Change a curve's parameter range or the U/V ranges of a surface without changing
its spatial shape. Start `Reparameterize` and pick a curve or surface, or select
the object before starting the command. Enter the domain start and end; a
surface then asks for its V start and end. Enter accepts the displayed current
value. `Automatic` is available at the first domain prompt. You can also enter
several values together, separated by spaces or commas:

```text
Reparameterize
Reparameterize -4 6
Reparameterize -2 8 10 30
Reparameterize Automatic
```

Single-face B-reps, including trimmed surfaces, holes, periodic seams and poles,
retain their trims. Click an individual polysurface face, use a preselected face,
or specify its face index. A whole polysurface selected before the command asks
for a face. `Object=id` targets a curve or surface without changing selection;
`Surface=id` remains a surface-only reference:

```text
Reparameterize Face=2 -2,8,10,30
Reparameterize Object=curve-id -4,6
Reparameterize Surface=object-id Face=2 Automatic
```

Only the addressed face's support and p-curves change. Spatial edges and
vertices, neighboring face charts, orientation, object identity, attributes,
groups and user text remain intact. The edit is one undo step; failures and
unchanged ranges preserve the redo branch. Changes survive 3dm export/import.
The command uses the shared validated affine
[face chart mapper](../fillet-parent-charts.md), including its refusal of charts
that lose excessive coordinate precision.

Viewport picks and partial numeric input do not edit the document. Escape,
invalid numbers and invalid ranges preserve the redo branch. A source geometry
or tolerance change ends the prompt. Curves over surfaces use the normal object
overlap chooser. Accepted faces are highlighted while entering their ranges.

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
cones, knot edits, chart edits, scales, weight gauges and document tolerances.
Five further public-command captures check Enter defaults and mixed typed/default
input on trimmed surfaces and a polysurface face. Shared source 3dm files
preserve the actual reference topology and parameterization. Both the command
and public `BrepFace.SetDomain` results are retained; its control-polygon size
query differs from the actual command on curved inputs. The reference is
SDK-only; replay runs through the Viboceros command tests, rather than a new
Rust oracle protocol operation. Application tests replay all 59 cases through
the numeric getter, and an egui pointer test checks front-face picking.
Automatic copying from block instances remains outside the current workflow.

```sh
cargo test --release -p viboceros-command --lib reparameterize
cargo test --release -p viboceros-command --test reparameterize_faces
cargo test -p viboceros app::tests::reparameterize
python3 -m unittest tools.rhino_oracle.test_reparameterize_face_probe
```

See the [Rhino command documentation](https://docs.mcneel.com/rhino/8mac/help/en-us/commands/reparameterize.htm),
[public face-domain API](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/M_Rhino_Geometry_BrepFace_SetDomain.htm),
the [face command validation](../reparameterize-faces-validation.json), and
[interactive input validation](../reparameterize-input-validation.json).
