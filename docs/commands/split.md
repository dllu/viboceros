# Splitting curves and surfaces

[Command reference](README.md) · [Project overview](../../README.md)

## Cutting objects and curve parameters

`Split point [point ...]` divides one selected curve at the closest curve
locations; omit the points to collect viewport picks and press Enter, or use

`Split Parameter=value[,value...]` for exact parameters. Open-curve pieces
remain in natural order. Closed curves produce cyclic pieces between the split
locations; a single location creates a full-loop polycurve containing both seam
pieces. Lines, arcs, polylines, and existing polycurve leaves retain their native
representations. Circles trim to arcs, while ellipses use parameter-equivalent
NURBS. Numeric locations use the [native source domain](../curve-domain-editing.md).
In these
point/parameter modes the first piece retains the source identity; every piece
retains its attributes and group membership, becomes selected, and participates
in one undo step. `Split CuttingObjects=source_point` instead chooses the
selected curve, NURBS surface, or rectangular single-face B-rep nearest that
point as the target and uses every other selected curve, surface, and B-rep as
an actual 3D cutter; in the UI, enter `Split CuttingObjects` and pick the source
in a viewport. Finite surface overlaps and exact B-rep trim regions, including
holes, supply curve split locations. Curve targets retain their native types and
parameterization. Wrapped closed pieces remain arcs, polylines, or NURBS rather
than the two-part seam polycurves of point/parameter mode. Closed seam hits count
once; a single closed cut leaves the source unchanged. See
[native cutting](../curve-cutting.md) for the parameter correspondence and tests.
A rectangular surface target is split at
every complete constant-U or constant-V intersection, with duplicate cuts
culled and multiple directions forming an exact UV grid. Its fresh single-face
B-rep results retain the complete underlying surface, the source face
orientation, attributes, and group membership. Partial isocurves do not divide
the target. Exact non-isoparametric intersections joining rectangle boundaries
may be combined with each other and with isoparametric cuts. Their planar UV
arrangement splits every cutter at transverse crossings, reuses shared corner
and intersection nodes, and emits every bounded cell with Rhino-compatible
vertex, edge, and trim ordering. Straight parameter paths work on arbitrary
rectangular NURBS faces. On affine non-rational tensor-product faces, curved
NURBS paths are pulled back exactly through degree elevation and knot
refinement, retaining their degree, control locations, weights, knots, and
domain. Homogeneous-affine rational tensor-product faces are likewise inverted
exactly without fitting; their UV weights absorb the surface's projective
denominator while degree, knots, domain, and model-space locus remain exact.
Other regular rectangular NURBS parameterizations use an adaptive piecewise-
cubic Hermite pullback whose every span is verified in model space at document
tolerance. Exact cubic parameter paths collapse to one span, while more
nonlinear planar or rational paths subdivide without changing the spatial edge.
Affine and genuinely warped non-rational bilinear faces also retain the source
structure of supported geometrically straight multi-span and higher-degree
cutters instead of simplifying them to a line. Rational trims
preserve their projective locus and interior junction data while normalizing
the outer Bezier weights to Rhino's canonical unit values. Curved paths are
accepted when each retained in-face control polygon advances strictly in at
least one UV direction. Warped patches use their independent bilinear twist
direction as an exact affine left inverse. Straight paths on higher-order
surface representations use Rhino's cubic four-control trim form while
retaining the source domain. General nonplanar trims are
constrained-triangulated in UV with
interior knot-span grid samples, so denser display settings refine both their
boundaries and surface interiors. Adjacent-side cuts produce Rhino's
triangle-and-pentagon topology in all four orientations.
Cuts from a corner to either nonincident side reuse the existing corner vertex
and produce exact triangle-and-quad topology; both opposite-corner diagonals
produce two exact triangles. A supported curved cut returning to the same
boundary side produces Rhino's two-edge lens in all four orientations. Its
complement has six edges for interior endpoints, five when one endpoint is a
corner, and four when it joins the side's two corners.
Simple closed cutters wholly inside the trim may be combined with one another
and with boundary-to-boundary cuts. Disjoint loops become multiple holes,
nested loops produce annular faces, and transverse cuts split both the closed
interiors and their complements. Pairs of polygonal loops, smooth rational
circles, or one of each may touch externally or internally at one vertex; they
retain Rhino's welded loop topology and tessellate without opening the contact,
independent of cutter order and direction. Smooth open and closed NURBS spans
remain intact between arrangement contacts, while degree-multiple tangent
kinks split into Rhino-compatible edges; self-crossing loops are rejected.
Boundary-to-boundary cutters may likewise touch a smooth
circle or polygon vertex without falsely crossing or opening the closed region.
Two open cutters may meet tangentially; regions pinched together only at that
contact remain distinct Rhino-compatible faces. Partially coincident cutters
are subdivided at both overlap boundaries and share one topological edge instead
of creating zero-area faces. Multiple disjoint shared spans between the same
pair retain each shared edge and the intervening lens faces with canonical seam
ordering across cutter order and direction. When coincident contributors differ
at an overlap endpoint, the tangent-continuous curve supplies Rhino's canonical
edge geometry and parameterization; otherwise the first equivalent contributor
is retained.
All contributor provenance is preserved for three-way overlaps, and open curves
provide the canonical edge when they overlap closed loops.
Straight and curved intersections against an already-trimmed rectangular
B-rep are partitioned at exact intersections with its four visible UV
boundaries. Globally nonmonotone cutters may enter the face repeatedly; every
disjoint supported simple span is retained with its exact spatial NURBS degree,
knots, weights, and subcurve domain. Rational clipped spans use Rhino's
piecewise-Bezier knot multiplicities and normalized outer weights.
For a non-affine surface, the p-curve may require an adaptive cubic fit.
Independent pullbacks of a clipped spatial edge can differ slightly at its
endpoints. The trim is clamped to the shared boundary locations when necessary,
and the adjusted curve is checked in model space across both its own knot spans
and the spatial edge's spans. Adjustments exceeding document tolerance fail.
The underlying surface and spatial edge remain unchanged.
Rhino-compatible cutting-object output replaces the source with fresh selected
pieces and leaves cutters unchanged and deselected. The curve fixture covers
curve, surface, solid box, holed planar-face, and view-aligned non-intersecting
cutters. The topology-aware surface fixture covers both isoparametric directions,
both opposite-side diagonal directions, all four adjacent-side directions, and
all ten nonincident corner-ending arrangements, same-side curved paths with
interior and corner endpoints in all four directions, closed polygonal and
rational-circle cutters in both directions, disjoint and nested rational-circle
pairs, mixed smooth/kinky loop orders, intersecting polygons, and line-crossed
multi-closed arrangements, single- and multi-span curved paths,
straight, curved, nonmonotone-control, and repeated-entry clipping on pretrimmed
sources, and parallel, crossing,
mixed-isoparametric, rational-curved, and shared-corner multi-cutter
arrangements, including collinear degree-one and degree-two multi-span paths,
degree-two paths with kinked linear spans, single- and multi-span rational
straight paths with nonunit endpoint weights, a linear path on a degree-elevated
planar surface, internally and externally touching closed polygons, and tangent
rational circles, plus mixed polygon/circle tangencies in both traversal
directions, open/closed tangent contacts in both cutter orders, and open/open
tangent contacts with pinched lobes, plus shared-start and shared-middle cutter
segments in swapped and reversed traversal orders, including an exact shared
quadratic span, open/closed shared edges, and adjacent closed polygons. It also
covers a reversed smooth continuation promoted over an earlier kinked cutter,
both smooth-promoted and all-kinked three-way shared spans, and degree-one and
genuinely curved degree-two open-cutter kinks in single- and multi-cutter
arrangements, plus disjoint repeated overlaps in swapped, reversed, and mixed
directions. It compares UV trim degree, controls, weights, knots, and domains as
well as vertices, edge domains, trim order,
metadata, grouping, and selection. The separate
`surface_split_nonaffine_trimmed.json` fixture compares fitted trims geometrically
at equal UV arc-length stations; see the [oracle documentation](../oracle.md).
Current surface/surface intersection support is planar.

## Isocurve splits

`Split Isocurve=point
Direction=U|V|Both Shrink=Yes`
splits exactly one selected untrimmed NURBS surface at the closest surface
location; omit the point after `Split Isocurve` for one viewport pick. The
direction names the isocurve itself, so a U isocurve divides the V parameter
domain and vice versa. The source is replaced by two or four fresh exact
single-face B-reps with matching attributes and group membership, selected
together in one undo step. Existing rectangular single-face B-reps can be
split again with the same command. `Shrink=Yes` clamps each underlying NURBS
surface to its result domain; `Shrink=No` retains the complete original surface
and stores the result as an exact rectangular UV trim. These nonplanar trims
tessellate, pick, bound, measure exact area, and round-trip through 3DM without
discarding the underlying surface. Closed directions reuse seam edges and
collapsed poles use singular trims. The topology-aware planar, cylindrical,
and spherical Rhino fixture agrees through vertices, edge domains, trim classes
and directions, face orientation, and geometry to within `2e-15`.
Curved paths through singular or self-overlapping parameterizations,
nonmonotone curved trim clipping, nonlinear
surface/surface cutters, and nonrectangular or multi-face B-rep sources remain
future extensions.
