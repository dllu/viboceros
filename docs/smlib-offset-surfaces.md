# Native surface offsets

Linux builds with `--features native-smlib` use usd-brep's offset and shell
operations for OffsetSrf's general geometry stage. The previous planar normal,
profile and rectangular-solid adapter is compiled only in other builds.
Canonical spheres, cylinders, cones and tori retain their qualified exact
fast paths; migrating those paths and general command parity remains ongoing.

The `Shape` bridge imports manifold sheets and closed solids through the same
NURBS payload writer. Closed-solid Boolean imports remain strict. Sheet import
uses explicit void-region topology and native SwapUV to encode reversed normals;
this can change parameterization and face-flip representation. Mixed open/closed
components and nonmanifold sheet inputs fail before model changes.

Native offset/shell operations work on private geometry with explicit creation
accuracy. Shared tolerance is restored on success and failure. Offset geometry
is transferred with an explicit NURBS approximation tolerance and checked native
achieved bound. Exact solid export continues to reject geometry that requires
approximation. This is a kernel-reported bound, not a whole-boundary proof.

BothSides creates separate offset sheets or a wall spanning the two offsets.
Sources remain untouched until all results are staged, and attributes/groups,
DeleteInput, selection and Undo/Redo retain the shared command flow. Surface
picking accepts polysurfaces in native builds and retains typed distance/options.

Five new [warped-patch SDK captures](../tools/rhino_oracle/observations/offset_warped_face_geometry.json)
qualify signed offsets, both sides and wall solids. Tests sample ideal normal
offset points against returned surfaces within 1e-8 and compare wall volume
against the recorded Rhino integrals within 2e-6. Existing planar and analytic
offset tests remain in place. These are public SDK observations, not an
interactive Rhino command capture or complete geometric equivalence proof.

Four [box SDK captures](../tools/rhino_oracle/observations/offset_box_geometry.json)
retain shifted solids and empty results. Rhino's SDK produces positive/negative
box offsets, but returns no bodies for the closed-box Solid requests. SMLib
constructs mathematical wall volumes for those requests; this remains a known
semantic difference pending command-level qualification. Broad self-intersecting
offsets, collapse policies, corner choices and performance parity remain open.
Positive box offsets also retain different edge partitions: native output has
12 edges and the recorded Rhino SDK result has 36. The
[partition ledger](smlib-offset-partitions.json) gates this difference separately
from volume and boundary-coordinate checks.
[Capture and replay provenance](smlib-offset-provenance.json) records retained
artifacts, tested source hashes and these limits.
