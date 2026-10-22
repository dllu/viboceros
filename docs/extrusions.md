# Extrusion representation and interchange

[Architecture](architecture.md) · [File formats](file-formats.md)

3DM extrusion objects retain their native type through import, affine transforms
and export. Their immutable OpenNURBS archive travels with a validated canonical
B-rep view, which supplies the existing display, snapping, selection, meshing and
modeling interfaces. Creating an ordinary B-rep does not mark it as an extrusion.

`Brep::try_planar_profile_extrusion` constructs this representation from a planar
NURBS profile, a frame, a nonzero height and a cap choice.
`Brep::try_from_extrusion_archive` restores it from an owned archive.
`Brep::extrusion` exposes the retained representation; `explicit_brep` removes it.
The geometry and IO layers share one versioned structural transfer codec.

At very large coordinates, native planar caps can report zero edge tolerance
despite a roundoff-sized gap between the trim image and its 3D edge. For matching
positive-weight NURBS bases, the transfer bounds that gap by the largest control
displacement and updates only the edge tolerance. Native validity checks still
apply; the repair declines larger gaps or mismatched bases. Regression cases
cover tilted cylinders through scale `2^80`, archive reloads and translation.
Affine invertibility checks exclude translation from native determinant pivoting,
so large translations cannot cause a false singularity.

Affine maps rebuild the archive and B-rep view together. Surface edits, UV swaps,
orientation reversal and topology reconstruction produce explicit B-reps, so a
later save cannot restore obsolete extrusion geometry. `ToNURBS` also produces an
explicit B-rep and follows its existing source-retention and Undo/Redo behavior.
Managed geometry user text stays independent of the archive, including removal.

The [qualification record](extrusion-interop-qualification.json) covers 11 native
Rhino files: open/capped rectangles, negative heights, circles, an arc, a kinked
profile, miters, shear and open/capped pipes. File identity, face orientation,
loop counts, rational control points, weights and knots, and 45 common-parameter
surface stations per face are checked.
Sampled errors stay within `1e-6`; continuous geometry error, every profile and
comparative speed remain unqualified. A misplaced inner-profile fixture has an
invalid native B-rep form and remains an unsupported-file diagnostic.

Native archives are limited to 64 MiB, with canonical transfers limited to
256 MiB. ToSubD extrusion conversion is available through `SubD::try_from_extrusion` and
the [command and application prompts](commands/to-subd.md).
[Creator representation and geometry](creator-extrusions.md) and command/prompt
comparisons are recorded separately from these file checks.
The [ToSubD kernel guide](tosubd-kernel.md) describes the completed mesh and
surface conversion APIs.

```sh
cargo test -p viboceros-geometry --release --test extrusion
cargo test -p viboceros-io --release --test extrusion -- --nocapture
cargo test -p viboceros-command --release --test extrusion
python3 -m unittest tools.rhino_oracle.test_extrusions
```
