# ShortPath

Enter `ShortPath`, select a surface or B-rep face, and pick the start and end
points. A single preselected surface is accepted. The second point shows a
nonmutating curve preview; clicking commits that cached result. Typed coordinates,
relative coordinates, document units and the shared surface point picker work in
the same getter. Escape cancels without adding geometry or consuming Redo.

Scripted examples:

```text
ShortPath 0,0,0 3,4,0
ShortPath 0,0,0 3,4,0 Surface=<object-id>
ShortPath 0,0,0 3,4,0 Surface=<brep-id> Face=2
```

Points project to the selected surface or face. The output uses the current
layer and has no name; it is unselected and the source selection remains.
Sources stay immutable. One Undo removes the output, and Redo restores it.
Changes to the captured source, tolerance, visibility/selectability or history
invalidate an interactive preview before commit.

The kernel reuses the existing geodesic integrator, adaptive cubic fitter and
continuous rational surface-image bounds. It proposes lines on planar patches,
unrolled helices on canonical complete cylinders, and great-circle arcs on
canonical spheres. A sphere pole's redundant longitude and both representations
of an exact periodic seam are handled during admission. Antipodal sphere paths
are nonunique; the canonical tie uses decreasing source U.

Other regular NURBS patches use bounded multiple-seed shooting, followed by a
cubic fit with fixed endpoints and endpoint tangents. UV axes and physical
integration coordinates are normalized. Every returned curve must pass a
continuous image bound over its complete domain, within half the document's
absolute tolerance. Integration and fitting error estimates do not certify a
global minimum. Query exhaustion, singular transitions and failed admission
return an error before editing the document.

This implementation finds the shortest converged candidate on a general patch.
It does not find every global branch, arbitrary pole crossing, or shortest route
that follows a natural boundary or detours around a trim hole. A trimmed face
admits a path only when the complete curve survives the shared trim clipper;
a crossing path is rejected. The `InfinitePlane` (`IP`) point option is not
implemented. These are limitations relative to the
[Rhino command](https://docs.mcneel.com/rhino/8/help/en-us/commands/shortpath.htm).

The source-pinned oracle corpus has 14 public `Surface.ShortPath` SDK cases and
four public command getter scenes, including cancellation. Native agreement
keeps the existing 1e-6 model gate. Curved native outputs can differ from the
analytic/numerical geodesic beyond that gate, so these are reported as
unqualified rather than treated as full compatibility. A separate unrolling
witness checks the captured cylinder discrepancy without a closest-point solver.
See [current measurements](../short-path-qualification.json).

```sh
cargo test -p viboceros-geometry --lib surface_short_path
cargo test -p viboceros-geometry --test surface_short_path_native -- --nocapture
cargo test -p viboceros-command --features native-smlib --lib short_path
cargo test -p viboceros-command --features native-smlib --test short_path_native -- --nocapture
cargo test --features native-smlib short_path -- --nocapture
python3 -m unittest tools.rhino_oracle.test_short_path
```
