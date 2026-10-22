# Surface intersection coverage

[Geometry guides](README.md) · [Intersection commands](../commands/editing.md#intersection-and-trimming)

`Intersect` compares every supported pair of selected curve-compatible objects,
untrimmed NURBS surfaces, and B-reps. Isolated and tangent contacts create
current-layer point objects, while finite shared intervals create exact NURBS
subcurves. Curve/B-rep contacts are clipped against exact face trim regions and
deduplicated across shared edges and vertices. Transverse planar surface pairs
produce exact, arc-length-parameterized lines clipped to both finite patches;
selection order determines their orientation as in Rhino. Nonplanar rational
bilinear patches with weights of one sign intersect finite planes in exact
conics, straight rulings, or isolated corner points, clipped to the plane patch.
Coincident nonsingular convex bilinear patches with weights of one sign, along
with certified affine or projective patches of any degree, produce an exact
shared edge or closed overlap perimeter, including Rhino's distinct edge
orientation and loop-domain rules. Injective planar strips with a monotone
parameter direction and separated ruled edges also return exact curved overlap
perimeters in either U or V orientation, including mixed-axis strip pairs and
their original NURBS control points, rational weights, and domains. Rational
strips with nonproportional weight rows are included when a one-sign Bernstein
Jacobian and separated monotone boundaries certify that the surface cannot
fold.
Supported surface/B-rep and B-rep/B-rep
intersections are clipped to exact face trim regions when needed, deduplicated
at shared edges and vertices, and joined into maximal components. Curved
surfaces against trimmed planar and spherical faces are included. Coincident
planar surface/B-rep and B-rep/B-rep pairs return the shared region perimeter,
including trim holes and curved cap boundaries. Multiple coincident planar
B-rep faces contribute each unique linear edge once and join the edge graph
into traversable curves.
## Spheres

Canonical spherical surfaces and planar surface patches intersect in exact
rational circles or circular arcs clipped to the finite patch. Exact tangency
creates a point. Rhino's surface intersection API can instead return tiny
numerical curves for this tangent case. The
[sphere/plane oracle fixture](../../tools/rhino_oracle/fixtures/sphere_plane_surface_intersection.json)
and [observations](../../tools/rhino_oracle/observations/sphere_plane_surface_intersection.json)
record full, clipped, tangent, and disjoint cases.
Two canonical spherical surfaces intersect in two exact rational semicircles,
matching Rhino's two-curve result for a secant pair. External and internal
tangencies produce exact points; disjoint and contained pairs produce no result.
The [sphere/sphere oracle fixture](../../tools/rhino_oracle/fixtures/sphere_sphere_surface_intersection.json)
and [observations](../../tools/rhino_oracle/observations/sphere_sphere_surface_intersection.json)
record secant, tangent, disjoint, and concentric cases. Rhino's numerical API
missed the internal tangent point in that fixture. Coincident spheres still
report an unsupported two-dimensional overlap.
Canonical coaxial spheres and finite cylindrical walls intersect in exact
rational circles, including tangent circles and sections on the cylinder rims.
Smooth noncoaxial sphere/cylinder branches are fitted as cubic curves within
the modeling tolerance and clipped at the cylinder rims. This includes the
single loop formed when the upper and lower branches meet at two radial turns.
Isolated rim and external radial contacts create points.
The [sphere/cylinder fixture](../../tools/rhino_oracle/fixtures/sphere_cylinder_surface_intersection.json)
and [observations](../../tools/rhino_oracle/observations/sphere_cylinder_surface_intersection.json)
record two circles, clipping to one, rim contact, tangency, and noncoaxial
full, clipped, rim-tangent, and radial-turning reference geometry. Rhino's surface API also
reports duplicate seam points on transverse curves and a tiny numerical curve
at the isolated rim tangency. The native result retains the exact circles and
point. Rhino splits the radial-turning loop into two open halves and misses the
external radial tangent in the saved observation. The native result retains one
closed loop and the exact tangent point. The exact singular case where two
branches cross is represented by an exact rational quartic figure-eight curve;
finite cylinder rims trim it to exact subcurves or an isolated contact point.
Canonical coaxial spheres and finite cone walls intersect in exact rational
circles, including tangent circles and sections on the cone's base rim. A
contact only at the singular apex produces no event. The
[sphere/cone fixture](../../tools/rhino_oracle/fixtures/sphere_cone_surface_intersection.json)
and [observations](../../tools/rhino_oracle/observations/sphere_cone_surface_intersection.json)
record these cases. Rhino's surface API missed the exact tangent circle and
reported redundant seam points alongside transverse circles in this fixture;
the native result retains the exact circles. When a noncoaxial sphere strictly
contains the cone apex, its intersection with the cone is a fitted cubic loop,
an arc clipped at the cone base, or an isolated base-rim contact. Spheres
through the cone apex produce fitted loops that may close at the apex, finite
arcs ending there, or isolated base-rim contacts. An apex-only contact is
omitted, following Rhino's surface API. Spheres outside the apex can produce
two separate fitted cubic loops; the cone base clips either loop to an arc or
isolated rim contact. At internal tangency the
two loops become one nodal cubic curve, passing twice through their contact;
the cone base can retain a closed lobe, an arc, or an isolated rim point.
When the two axial branches meet at radial turns, they produce two fitted open
curves, clipped at the base or reduced to one tangent point. This includes
sections whose opposite cone generator intersects the sphere only at negative
cone heights. The saved noncoaxial fixture covers a turning section; its two
native branch lengths agree
with Rhino within 2×10⁻⁶. Rhino also reports redundant seam points there.
An external tangency creates one point, clipped to the finite cone height.
Other noncoaxial sphere/cone sections remain unsupported.
## Cylinders

Canonical cylinders intersect perpendicular planar patches in exact circles
and parallel patches in straight generatrices. Oblique sections are exact
rational ellipses clipped to both finite surfaces. Rhino returns cubic fitted
curves for those sections; the native rational curves follow the same geometry.
At a tangent parallel plane, the two coincident generatrices are retained to
match Rhino's result. The
[cylinder/plane fixture](../../tools/rhino_oracle/fixtures/cylinder_plane_surface_intersection.json)
and [observations](../../tools/rhino_oracle/observations/cylinder_plane_surface_intersection.json)
cover those cases.
Parallel canonical cylinder walls intersect in exact, height-clipped
generatrices. External tangencies retain two coincident lines and internal
tangencies retain four, matching Rhino's surface API. Rim-only transverse
contacts produce points; coaxial walls sharing one rim produce its exact circle.
Coaxial walls overlapping over an area produce no API events, as Rhino does.
The [cylinder/cylinder fixture](../../tools/rhino_oracle/fixtures/cylinder_cylinder_surface_intersection.json)
and [observations](../../tools/rhino_oracle/observations/cylinder_cylinder_surface_intersection.json)
record these parallel cases.
Equal-radius cylinder walls whose axes cross intersect in two exact rational
ellipses, including oblique crossings. Both finite height ranges clip them to exact conic
arcs or isolated rim points. Unequal-radius walls with crossing axes produce
two cubic curves fitted within the modeling tolerance, including oblique
crossings. Finite heights clip them to arcs and retain isolated rim contacts.
Skew-axis cylinders with disjoint finite wall bounds report no intersection.
When the larger radius exceeds the smaller radius plus the axis separation,
their walls produce two separate fitted cubic curves; finite heights clip the
curves to arcs or isolated rim contacts. When the axis separation lies between
the radius difference and the radius sum, the walls produce one connected
fitted cubic loop, likewise clipped to finite heights. External tangency
produces an isolated point. At internal tangency, the fitted cubic curve makes
two turns through one crossing point; finite heights can leave arcs or isolated
rim contacts.
## Cones and cylinders

Coaxial canonical cone and cylinder walls meet in one exact rational circle
when the cylinder radius occurs within both finite height ranges. Opposed
surface axes yield two exact semicircles, matching Rhino's event structure.
The [cone/cylinder fixture](../../tools/rhino_oracle/fixtures/cone_cylinder_surface_intersection.json)
and [observations](../../tools/rhino_oracle/observations/cone_cylinder_surface_intersection.json)
cover aligned and opposed axes, rim contacts, clipping, and a noncoaxial pair.
Rhino also reports two redundant seam points with the full circle; the native
result retains the circle. With parallel offset axes, smooth intersections are
fitted as cubic curves within the modeling tolerance and clipped to both finite
surfaces; isolated rim contacts create points. A cylinder wall through the cone
apex produces a cubic curve with an exact corner there. Full sections can be
closed; finite rim cuts produce arcs and an apex-only contact produces no event.
When a perpendicular cylinder axis passes through the cone apex, the walls
intersect in a fitted cubic loop. Both finite height ranges clip it to arcs or
isolated rim contacts; a cone base tangent to the loop creates two points.
Other nonparallel cone/cylinder axes remain unsupported.

Coaxial canonical cone walls meet in an exact rational circle when their
linear radius profiles cross inside both finite height ranges. This includes
opposed axes and a circle shared by both base rims. An apex-only contact is
omitted; coincident wall regions remain unsupported. Parallel offset cones
with equal slopes (within modeling tolerance) use a conic plane
section, clipped to both finite height ranges. When their bases meet in only
one axial plane, isolated contacts are returned as points. Parallel offset cones
with unequal slopes produce two cubic branches fitted within the modeling
tolerance, clipped at both cone bases; a single tangent contact produces a
point. Cones with different axis directions and a shared apex intersect in
zero, one, or two exact finite generators. Other nonparallel cone pairs remain
unsupported.

## Tori

Canonical ring tori intersect planes perpendicular to their axes in two exact
rational circles, or one tangent circle at the tube's top or bottom. Planes
containing the axis cut two exact tube circles. Finite planar patches clip
these circles to arcs or points. Axis-parallel offset planes produce one or two
cubic loops fitted to the modeling tolerance and clipped to the finite patch;
the inner tangent plane produces two pinched loops meeting at one point.
Nearby offsets retain their distinct one-loop or two-loop topology. Oblique
planes produce one or two cubic loops fitted to the modeling tolerance, with
finite patches clipping them to arcs. Critical oblique cuts retain crossing
loops, while isolated oblique tangencies return points. Shallow tilted cuts
near an axis-containing plane retain stable full meridian loops, including
finite-patch clipping. The
[torus/plane oracle fixture](../../tools/rhino_oracle/fixtures/torus_plane_surface_intersection.json)
covers perpendicular, axis, oblique, finite-patch, and disjoint cuts. Coaxial torus and finite
cylinder walls meet in one or two exact rational circles, including tangent
sections and circles on cylinder rims. Parallel offset cylinder axes produce
fitted cubic loops, finite rim-clipped arcs, or isolated contact points.
Critical sections retain their single, pinched, and crossing loops. Cylinders
whose perpendicular axes cross the torus center and whose radii fit inside
the torus's inner rim produce four cubic loops, finite rim arcs, or isolated
rim contacts. The tube-radius case retains crossing curves. At the inner rim
radius, two crossing loops with a doubled period join the four-loop and two-loop
regimes. Larger centered perpendicular cylinders produce two turning loops
and finite arcs up to the outer tangent radius, which gives two contact points.
The same centered-axis construction also handles thick-tube ring tori when the
cylinder radius lies between the inner rim and tube radius, with separate outer
and inner loops and finite rim clipping. At the inner rim radius, the inner
curves cross. At the tube radius, two turning loops cross. Other nonparallel
torus/cylinder axes remain unsupported. The
[torus/cylinder oracle fixture](../../tools/rhino_oracle/fixtures/torus_cylinder_surface_intersection.json)
covers exact circles, offset loops, finite arcs, crossings, and contacts.
Spheres centered on a torus axis intersect it in one or two exact rational
circles, including tangent circles. Offset spheres produce fitted cubic loops
or isolated tangent points. The
[torus/sphere oracle fixture](../../tools/rhino_oracle/fixtures/torus_sphere_surface_intersection.json)
covers axial, offset, tangent, contained-meridian, and disjoint cases.
Coaxial tori likewise intersect in one or two exact rational circles, including
tangent circles. Equal tori with parallel offset axes intersect along a
symmetry-plane section and an elliptic section. Tori with equal tube radii,
unequal major radii, and parallel offset axes at the same axial level intersect
along lifted hyperbolic and elliptic sections. These produce fitted cubic loops,
pinched loops, exact meridian circles at conic collapse, or isolated contacts.
Equal tori with a common center and different axis directions intersect in two
planar sections, also returned as fitted cubic loops. Coincident tori, unequal
tube radii with offset axes, axial offsets, and other intersecting nonparallel
pairs remain unsupported. Separated surface control hulls return no intersection
events. The
[torus/torus oracle fixture](../../tools/rhino_oracle/fixtures/torus_torus_surface_intersection.json)
covers coaxial circles, parallel offsets, tangency, and disjoint cases.
Coaxial tori and finite cone walls intersect in up to two exact rational
circles, clipped to the cone height. Tangent circles and cone rims are included;
offset and nonparallel torus/cone axes remain unsupported.
The [torus/cone oracle fixture](../../tools/rhino_oracle/fixtures/torus_cone_surface_intersection.json)
covers finite coaxial circles and disjoint cases.

## Cone/plane sections

Canonical cones intersect planar patches in exact circles, rational elliptical,
parabolic, and hyperbolic arcs, or straight generators. A plane touching only the
singular apex produces no result, matching Rhino. The two coincident tangent
generators are retained. Rhino fits some conics as cubics, so its curve lengths
can differ slightly from the exact rational sections. The base rim tangent
is returned as one exact point; Rhino's numerical intersection
returns two tiny curves there. The
[cone/plane fixture](../../tools/rhino_oracle/fixtures/cone_plane_surface_intersection.json)
and [observations](../../tools/rhino_oracle/observations/cone_plane_surface_intersection.json)
record the reference cases. Finite arcs are retained when the full projective
conic has a pole outside the cone. Sections without two distinct base-rim
crossings can still be unsupported.

## Orientation, selection and remaining limits

Curve/curve overlaps use the later curve's orientation and parameterization,
matching Rhino. Pairwise duplicates are intentionally retained when three or
more source objects meet at one location. Inputs remain in the document and are
deselected, outputs are selected, and all output creation is one undo step. A
no-hit run still clears the input selection but creates no undo record.
Other non-planar and more general coincident surface/surface intersections,
curved B-rep face pairs, and multiple coincident curved face regions remain
future extensions.

