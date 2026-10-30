# Native mesh Cap comparisons

[Command usage](../commands/cap.md) · [Geometry guides](README.md)

Twenty-two fresh Rhino command captures retain authored input recipes, exact
source 3DM archives, public mesh validity/storage witnesses, command-end state,
post-script state and Undo/Redo. All input vertex arrays survive capture and
archive replay exactly. The cases include ordinary and narrow openings, winding,
large coordinates, multiple sources, partial groups, welded copies and no-ops.
Command and application replays share comparison code and scene decoding fixtures.

`Cap DeleteInput=No` creates ungrouped copies with the source attributes. It
preserves the original memberships and selection at command end. The document
copy adapter now stages those outputs atomically without editing source groups;
existing grouped-copy adapters retain their policies.

Rhino accepts nondegenerate cap facets below the document's feature-size floor.
Hole filling, planar caps and cap-boundary welding now validate defined facets
numerically. The document tolerance still controls planarity. Zero edges and
zero-area triangles remain invalid. Default-tolerance analytic regressions cover
fine facets, source colors/UVs, copy options, winding and history.

## Remaining differences

All captured metadata, source identity, n-gon boundaries and history transitions
match. Stored vertex/face counts still differ in nine scenes, including simple
caps where Rhino adds interior center vertices. Nine count differences do not
imply nine surface differences.

Five large-scale captures return separate caps with non-solid topology despite
Rhino reporting success. Their surface/topology differences remain explicit;
public `IsValid` is true and does not establish a correct closed region.
Rhino flips the reversed narrow scene to outward winding; the current kernel
cap retains source winding. Its sampled geometry still agrees within `1e-6`.
Stored normal values differ in that scene and two welded-copy scenes. Nine
scenes with different counts have no normal-array bijection comparison.

The conservative boundary comparison covers convex faces in axis-aligned planes,
keeps opposite orientations separate, and declines ambiguous or unsupported
faces. It provides exact coordinate-boundary agreement in sixteen scenes.
Seventeen scenes agree under bidirectional sampling at the unchanged `1e-6`
bound. These are separate measurements, not a general continuous-surface bound.

Source arrays are identical at large scale, but the current closest-point sampler
can report nonzero error on the long thin source faces. That query-precision gap
is retained separately and needs a kernel fix. Arbitrary frames, SubD Cap and
comparative Rhino performance remain unqualified.

The [validation record](cap-mesh-native-validation.json) binds these captures,
current sources, checks and every retained gap. Rhino's public
[Cap reference](https://docs.mcneel.com/rhino/8mac/help/en-us/commands/cap.htm)
describes the copy, crease and triangle options used by the captures.
