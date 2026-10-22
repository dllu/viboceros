# Explicit SDK box topology

`Brep::try_command_box` follows the public
OpenNURBS box layout: eight perimeter-ordered vertices, twelve directed edges,
and faces ordered Y-min, X-max, Y-max, X-min, Z-min, Z-max in the construction
frame. Each face retains its outward orientation and South/East/North/West
trim flags and reversals.

Surface U/V intervals start at zero and span the maximum length of their
opposite edges. Model edges and UV trim curves also retain native length
intervals. For lengths at or below the OpenNURBS zero threshold, parameter
extents fall back to one; model-space coordinates remain unchanged.

The topology/extent tables are an attributed Rust adaptation in
`third_party/opennurbs_rust/box_topology.rs`, under that directory's MIT license.
Sources are `ON_BrepBox`, `ON_NurbsSurfaceQuadrilateral`, and `ON_LineCurve` at
pinned OpenNURBS revision `23fc677ba06e49212296ca75fab7fb6c2851b4ce`.
The Rust kernel independently validates frame intervals and constructs the
geometry and shared topology. All work precedes document admission.

`Brep::try_box` remains the generic normalized constructor. Public-SDK box
fixtures use `try_command_box` to retain matching face order and UV intervals.
The `Box` command now creates a [typed extrusion](creator-extrusions.md); its
canonical B-rep can have different seams and face ordering. This guide records
the explicit B-rep API and its original qualification.

Regression coverage includes connectivity, oriented faces, physical surface
and trim domains, a rotated frame, tiny-box fallback, invalid intervals,
source-preserving Box-to-Contour behavior and Undo. Tiny geometry is checked
with a scale-appropriate modelling tolerance; the constructor does not silently
relax the caller's tolerance. The existing 24 native B-rep contour captures
remain a strict pass through the new constructor.

Broader box command options, degenerate collapsed boxes, general affine frames
and performance parity remain outside this checkpoint. Full Rhino command and
geometry compatibility is still in progress.
