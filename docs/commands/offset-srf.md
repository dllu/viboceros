# OffsetSrf

[Command reference](README.md) · [Rhino command](https://docs.mcneel.com/rhino/8/help/en-us/commands/offsetsrf.htm)

Select one or more planar surfaces, exact canonical spheres, cylinders, cones,
or tori, or supported single-face B-reps. Run `OffsetSrf 2` or
`OffsetSrf Distance=2`. Planar outputs are translated along the face normal.
Spheres and cylinders change radius; cones open their apex into a circular
rim; tori change minor radius. A negative
distance offsets in the opposite direction. Trimmed planar B-reps
retain their exact curves and topology. Outputs inherit their source
attributes and groups and are selected.

`BothSides=Yes` creates one disjoint two-face B-rep with offsets at both
signed distances. For planar sources, `Solid=Yes` makes a capped extrusion
from each source to its offset when the source has one closed boundary loop.
Rectangular bilinear surfaces produce six planar faces; other profiles retain
an exact ruled wall and two caps. Together,
`BothSides=Yes Solid=Yes` makes one solid spanning both offsets.
`DeleteInput=Yes` removes the sources. All three options default to `No`.
One Undo restores the prior document. The command stages all outputs before
editing the document, so unsupported selections
leave it unchanged.

Planar offsets are exact translations. The current solver rejects nonplanar
surfaces other than canonical spheres, cylinders, cones, and tori, multi-face B-reps,
and solid planar offsets with multiple boundary loops. Canonical spheres use their exact
rational control net: single-sided offsets change the radius, two-sided open
offsets create one two-face B-rep, and solid offsets make a concentric
two-face shell. Offsets that collapse or invert the sphere are rejected.
Canonical cylinders preserve the exact rational wall control net and its
height. Two-sided open offsets create one two-face B-rep. Solid offsets
create a four-face tube whose annular caps each have a radial seam, matching
Rhino's offset-face topology. Offsets that collapse or invert the cylinder
are rejected.
Canonical cones preserve their rational weights and parameter domains. Their
open offsets are exact ruled surfaces at the requested normal distance, and
two-sided open offsets create one two-face B-rep. Solid cone offsets join the
source and offset walls with exact ruled caps into a four-face shell. The apex
cap retains a singular vertex for a one-sided offset; both sides use two rims.
Canonical ring tori retain their major radius and source parameter domains
while changing the minor radius. Open two-sided offsets create one B-rep
containing two closed faces; solid offsets reverse the inner torus to make a
two-face shell. Offsets that collapse the minor radius or make it reach the
major radius are rejected.
Other analytic parameterizations, free-form offsets, polysurface corner joining,
loose offsets, and interactive direction arrows remain to be implemented.
Tests cover signed and two-sided offsets, trimmed face orientation, exact
planar solid volume, Undo, and
atomic rejection. The [public Rhino offset-face probe](../../tools/rhino_oracle/fixtures/offset_surface_face_geometry.json)
and [Rhino 8 observations](../../tools/rhino_oracle/observations/offset_surface_face_geometry.json)
record face counts, bounds, topology counts, and signed solid volume for six
rectangular cases. Rhino reverses the inward negative solid when adding it to
the document, yielding positive volume; Viboceros follows that document
admission rule. This public API probe does not establish the interactive
command's selection or option-prompt behavior.

The [sphere offset probe](../../tools/rhino_oracle/fixtures/offset_sphere_face_geometry.json)
and [Rhino observations](../../tools/rhino_oracle/observations/offset_sphere_face_geometry.json)
cover positive, negative, two-sided, and solid cases. The native regression
checks exact radii and the corresponding shell volumes.

The [cylinder offset probe](../../tools/rhino_oracle/fixtures/offset_cylinder_face_geometry.json)
and [Rhino observations](../../tools/rhino_oracle/observations/offset_cylinder_face_geometry.json)
cover positive, negative, two-sided, and solid cases. The solid records retain
all eight edges and four face loops, including the two cap seams.

The [torus offset probe](../../tools/rhino_oracle/fixtures/offset_torus_face_geometry.json)
and [Rhino observations](../../tools/rhino_oracle/observations/offset_torus_face_geometry.json)
cover five open and solid cases. Its recorded U and V domains remain those of
the source torus for every offset.

The [cone offset probe](../../tools/rhino_oracle/fixtures/offset_cone_face_geometry.json)
and [Rhino observations](../../tools/rhino_oracle/observations/offset_cone_face_geometry.json)
cover both signed directions and a two-sided open result. A separate
[solid probe](../../tools/rhino_oracle/fixtures/offset_cone_face_solid_geometry.json)
and [observations](../../tools/rhino_oracle/observations/offset_cone_face_solid_geometry.json)
record one-sided positive and negative shells and a two-sided shell, including
their exact topology and volume.
