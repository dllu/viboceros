# IPlane / InfinitePlane

`IPlane` (`IP` or `InfinitePlane`) intersects curves, surfaces, B-reps and meshes
with an unbounded plane. Inputs remain in the model. New curves and point
contacts use the current layer and form one Undo/Redo step.

In the application, select inputs before starting IPlane, or start the command,
pick inputs and press Enter. Choose a World/CPlane axis plane, a planar object
or face, a saved CPlane, or draw a plane. Accepting the plane executes the cut.
Enter at the initial plane choice uses CPlaneXY. The plane definition does not
create a surface or change the viewport's construction plane.

The picker is shared with [Project's infinite-plane input](project-infinite-plane.md):
Rectangle, 3Point, Vertical, ThroughPoint, Object, AroundCurve and NamedCPlane
use the same drafting and reference rules. Point input supports snapping, units,
filters and constraints. Undo revises a pending point; Enter during an incomplete
point sequence returns to plane choice. Esc restores the original selection.
Failed geometry execution leaves the plane choice available for a retry.

Headless examples:

```text
IPlane Plane=WorldXY
IPlane Objects=object-id,other-id Plane=0,0,2,0,0,1
IPlane Objects=object-id Plane=Object:reference-id:face-index
IPlane Objects=object-id Plane=AroundCurve:curve-id:parameter
```

Omitted Objects uses eligible selected objects; explicit IDs must be selectable.
Duplicate IDs cut once. Omitted Plane uses CPlaneXY from the command context.
The six coordinates specify a world origin and nonzero normal. Curve parameters
use the reference curve's native domain. One plane is accepted per invocation.

The implementation reuses [Section](section.md): mesh cuts use its exact dyadic
edge predicates and topology tracing; other cuts use its validated intersection
producer, including usd-brep when enabled. Trim holes split output intervals.
Tangencies return point contacts, and supported coplanar overlaps return curves.
Results stage before document edits. Existing source/output budgets apply.

Full Rhino parity and comparative performance remain unqualified. General curved
intersections retain the underlying producer's supported-family limits. Extreme
coordinates can decline when the bounding cutter cannot be represented. Points,
point clouds, instances, SubD and clipping-plane objects are not supported inputs.
Named-plane icon/list picking and closed-mesh seam fidelity remain open. The
local picker sequence has not been checked against a fresh Rhino getter.

The Python oracle operation `iplane` takes `sources`, `origin` and `normal`,
using the existing owned object-source schema. Viboceros executes the command;
Rhino uses public [CurvePlane](https://developer.rhino3d.com/api/rhinocommon/rhino.geometry.intersect.intersection/curveplane),
[MeshPlane](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/M_Rhino_Geometry_Intersect_Intersection_MeshPlane.htm)
and [BrepPlane](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/M_Rhino_Geometry_Intersect_Intersection_BrepPlane.htm).
These SDK calls compare geometry rather than native command/getter behavior.
A failed BrepPlane call declines even if it returns partial curves. All owned
SDK geometry is disposed on success and failure. Curves are sampled at 32 equal
arc-length intervals (33 points for open curves, 32 for closed curves);
closed-curve seams are not canonicalized. Timings include
command/document work in Viboceros and SDK/sampling work in Rhino, so they do not
establish kernel speed parity. Fresh Rhino capture remains blocked by the
[diagnosed Wine security-library failure](../oracle-sspi.md).

```sh
cargo test -p viboceros-command iplane
cargo test -p viboceros app::tests::project
python3 -m tools.rhino_oracle viboceros tools/rhino_oracle/fixtures/iplane.json
python3 -m unittest tools.rhino_oracle.test_iplane_probe
```

See the [qualification record](../iplane-qualification.json) and
[Rhino command reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/iplane.htm).
