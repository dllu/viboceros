# DivideAlongCreases

`DivideAlongCreases [SplitAtKinks=Yes|No] [SplitAtTangents=Yes|No]`

Select surfaces or polysurfaces before running the command, or start it, pick
objects, and press Enter. During picking, the two Yes/No options can be typed or
clicked; Esc cancels. Both options initially default to Yes and are remembered
by the command registry. Separate registries have independent preferences.

`SplitAtKinks` divides at native position-continuous, tangent-discontinuous
locations. `SplitAtTangents` divides at native curvature discontinuities with
continuous tangent directions. These choices work independently. Knot
multiplicity alone does not determine a split: a geometrically smooth surface
with a parameter-speed change remains intact.

The operation requires `native-smlib`. It delegates continuity classification,
exact tensor isocurve extraction, UV segment creation and face subdivision to
usd-brep. New faces remain joined in the same object with their trimmed regions
and shared edges. Division adds boundaries on existing surface geometry; it does
not fit a replacement surface. The faces can retain unused portions of the
underlying surface; [ShrinkTrimmedSrf](shrink-trimmed-surfaces.md) can reduce them.

The adapter supplies independently selected U/V cuts instead of using the pinned
kernel's `SubdivideFaceAtDiscontinuities` convenience method, which uses the U
cut count for both directions. It requests the [repaired native isocurve pair](../native-isocurve-pairs.md)
directly, retaining the complete curve basis and its UV-coordinate line. This
replaces the adapter's UV construction workaround and preserves the exact B-rep
export gate. Upstream sources remain unchanged; repairs compile generated copies.

Surviving object IDs, attributes, groups and selection are retained. All selected
objects are prepared before one transaction; a failure leaves every source
unchanged. Undo/Redo replays the whole division. No-ops keep Redo available and
retain their original surface representation. Turning both options off is a
no-op in every build. Prepared edits reject changed source geometry or document
tolerance.

Resource limits are 512 input faces, 2,000,000 input/output surface controls,
2,048 unique knots per direction, 1,024 split lines and 16,384 output faces per
B-rep. Continuity uses the native kernel's geometric criteria; the document's
absolute tolerance controls face subdivision. These checks do not certify
arbitrary self-intersecting or position-discontinuous surfaces.

The [qualification record](../divide-along-creases-qualification.json) records
U/V kinks, tangent jumps, crossing creases, smooth knots, rational weights,
asymmetric parameter domains, remote origins, partial rectangular trims and a
crease crossing a hole on either side of a sheet. Tests cover areas, shared
topology, hole Euler characteristic, oriented world normals, UI options and
history. The 17 local command recipes compare independent topology/area
expectations. The owned Rhino 8 SDK probe calls `SplitKinkyFaces` and
`SplitFacesAtTangents` on B-rep copies; it does not simulate document getters,
metadata or history. Fresh licensed captures, full Rhino parity and comparative
performance remain unqualified; see the recorded [Wine startup issue](../oracle-sspi.md).

References: [Rhino command](https://docs.mcneel.com/rhino/8/help/en-us/commands/dividealongcreases.htm),
[tangent splitting API](https://developer.rhino3d.com/api/rhinocommon/rhino.geometry.collections.brepfacelist/splitfacesattangents).
