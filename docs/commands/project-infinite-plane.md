# Project onto infinite planes

During Project's target selection, type `IP`, `IPlane` or `InfinitePlane`.
Choose a world/CPlane axis plane, define a plane with points, use a planar object
or face, choose a plane perpendicular to a curve, or enter a saved CPlane name.
Plane targets have no finite boundary and create no temporary model surfaces.
They can be combined with ordinary surface, B-rep and mesh targets.

The picker offers:

- `WorldXY`, `WorldYZ`, `WorldZX`, `CPlaneXY`, `CPlaneYZ`, `CPlaneZX`. Enter at the
  initial choice uses CPlaneXY. CPlane-based choices capture the current frame
  when accepted; later viewport changes do not move an accepted target.
- `Rectangle`: first and opposite corners define a plane parallel to the
  construction plane through the first corner.
- `3Point`: three noncollinear points define the plane.
- `Vertical`: first and adjacent corners establish the vertical plane; a third
  point supplies its nonzero height.
- `ThroughPoint`: one point defines a plane parallel to the captured CPlane.
- `Object`: select a planar curve, surface or single polygon mesh. For a mesh
  or multi-face B-rep, type `object-id Face=index` to choose a planar face.
- `AroundCurve`: select a curve and pick a point on it. Its tangent defines the
  plane normal.
- `NamedCPlane`: type a saved CPlane name, optionally enclosed in quotes. This
  reads the saved plane without restoring its viewport or grid settings.

Point modes use shared snapping, filters, constraints, units and typed
coordinates. Undo revises a plane point without changing model history. Invalid
points keep the getter active. Enter abandons an incomplete plane while keeping
previously accepted targets; Esc cancels Project and restores its initial
selection. Choosing a different Project direction abandons an incomplete plane.
Repeat IP to add more planes, then Enter at target selection to project.

The headless command accepts repeated plane definitions:

```text
Project InfinitePlane=WorldXY
Project InfinitePlane=0,0,2,0,0,1 Direction=1,0,2
Project InfinitePlane=Object:object-id[:face-index]
Project InfinitePlane=AroundCurve:curve-id:parameter Direction=1,0,0
```

The six-number form gives a world origin followed by a nonzero normal. CPlane
keywords use the supplied command context. Plane arguments add to ordinary
Targets or selected surface targets. Leave those surfaces unselected when
projecting solely to an infinite plane. Explicit curve parameters use the
source's native domain.

OutputLayer=TargetObject uses a referenced object's layer; generated and named
planes use the current layer. DeleteInput and temporary SubCrv sources retain
their existing behavior. All results stage before one Undo/Redo edit. A parallel
direction is a miss. Collapsed curve images decline atomically.

Projection reuses the existing affine curve map, with the shared compensated
fixed-origin transform constructor. Point and common-sign-weight control images
are checked against the ideal directional projection at the absolute model
tolerance. The shared outward interval arithmetic admits well-conditioned
images; uncertain cases compare squared Euclidean errors with exact rationals.
Common-sign weights give a convex bound on the complete curve's image error.
Difficult conditioning can therefore decline
instead of admitting an image outside tolerance. General numerical conditioning,
native getter parity and comparative performance remain unqualified.

The current picker uses a text choice and typed named-plane selection. Named
CPlane axis-icon/list picking, clipping-plane objects and standalone IPlane are
pending. These local behaviors have not been captured from a live Rhino getter.

The Python `project_curve_plane` operation accepts a NURBS `curve`, world
`origin`, nonzero `normal` and nonparallel `direction`. It returns complete
projected curve definitions. Its SDK counterpart uses public
[Transform.ProjectAlong](https://developer.rhino3d.com/api/rhinocommon/rhino.geometry.transform/projectalong)
and rejects the SDK's identity fallback. Parallel comparison inputs decline;
the full Project command still treats parallel targets as misses. Four recipes
retain native and portable outputs, checked against independent exact rational
control-point projections. Fresh Rhino output remains unavailable because of
the diagnosed Wine security-library failure.

```sh
cargo test -p viboceros-command --features native-smlib project::tests
cargo test -p viboceros app::tests::project
python3 -m tools.rhino_oracle viboceros \
  tools/rhino_oracle/fixtures/project_curve_plane.json --native-smlib
python3 -m unittest tools.rhino_oracle.test_project_plane
```

See the [qualification record](../project-infinite-plane-qualification.json),
[Project reference](project.md), and
[Rhino InfinitePlane documentation](https://docs.mcneel.com/rhino/8/help/en-us/commands/iplane.htm).
