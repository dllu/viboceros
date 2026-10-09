# OpenNURBS Rust adaptations

`circle_transform.rs` is an altered Rust adaptation of the radius policy in
`ON_Circle::Transform`, with the plane construction supplied by Viboceros' kernel.
It is used only by AddObjectsToBlock to reproduce Rhino's circular approximation
under nonuniform placement. General affine editing continues to preserve exact
conics through NURBS conversion.

Source: the vendored OpenNURBS 8.x `opennurbs_circle.cpp` and
`opennurbs_plane.cpp`. Original copyright belongs to Robert McNeel & Associates;
the distribution's license notice is retained in LICENSE. This is not proprietary
Rhino code and is not the original OpenNURBS distribution.
