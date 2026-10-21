# MeshPatch

`MeshPatch` constructs a triangle mesh from curves, point objects and point
clouds. Select the sources first, or start the command and select them. Enter
continues to hole selection; select closed curves for holes, then Enter builds
the mesh. `StartingSurface` selects a surface whose UV chart guides triangulation.
Escape cancels the staged selection without consuming Redo.

```text
MeshPatch
MeshPatch AngleTolerance=15 TrimBack=No MainSegmentCount=10
MeshPatch Sources=<id>,<id> Holes=<id> StartingSurface=<id>
```

`AngleTolerance` is in degrees, between 0 and 180. It controls refinement of
curved inputs; existing polylines retain their vertices. `TrimBack` defaults to
Yes. `MainSegmentCount` defaults to 10 and applies to an untrimmed rectangular
perimeter. Numeric preferences belong to the command registry and survive model
Undo. Independent registries start with their own defaults.

Without a starting surface, the command reuses the least-squares plane fitter.
The triangulation preserves the original spatial samples. With `TrimBack=No`,
the fitted rectangle extends each planar bound by ten percent. A starting
surface instead supplies its natural rectangular UV perimeter.

Automatic trimming retains guide regions containing input points. When there
are no input points, it retains enclosed guide regions. Empty regions and
temporary perimeter faces are removed; explicit hole curves remove their
interiors. Thus adding a point inside a closed guide can fill a region that
would otherwise remain empty. Open guides can also remove adjacent faces.
This follows the captured native behavior; a convex hull alone does not
reproduce it.

Scripts can provide `Sources`, `Holes`, and `StartingSurface` object IDs. A
multi-face starting B-rep requires `Face=<zero-based-index>`. `OuterBoundary`
provides an explicit closed outer polyline and bypasses automatic guide-region
trimming. References remain unchanged. The result has no name, is unselected,
and uses the current layer. Source selection is released. One Undo removes the
mesh, and Redo restores it.

Construction is bounded to 100,000 input/perimeter records. Crossing constraints,
conflicting samples at the same chart location, open hole boundaries and
unresolved approximations fail before geometry edits. Angular approximation
rejects rational poles and knot discontinuities, and uses existing exact Bézier
extraction and splitting with a bounded refinement budget. It controls direction
variation, not a universal positional distance to the original curve.

The [qualification report](../mesh-patch-qualification.json) records 21 public
Rhino command/SDK scenes, including one matching construction failure. Geometry
comparisons retain the `1e-6` model gate and check metadata, immutable sources and
Undo/Redo. Full compatibility is unproven: general curved guide sampling,
periodic or folded starting charts, intersecting constraints, trim boundaries
of starting B-reps, native triangle ordering and performance need broader
qualification. These sampled comparisons do not certify universal mesh topology.

References: [McNeel MeshPatch](https://docs.mcneel.com/rhino/8/help/en-us/commands/meshpatch.htm)
and [public Mesh.CreatePatch API](https://developer.rhino3d.com/api/RhinoCommon/html/M_Rhino_Geometry_Mesh_CreatePatch.htm).
