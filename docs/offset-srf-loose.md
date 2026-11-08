# Loose surface offsets

`OffsetSrf Loose=Yes` offsets free-form single surfaces in either build. The
direction getter accepts `Loose=Yes|No` and a `Loose` toggle. Its value survives
cancellation. Distance, units, flips, two-sided skins, solid walls, source
replacement, Undo/Redo and 3DM persistence follow the ordinary command workflow.

The operation interpolates normal displacements at the original Greville
stations with the original rational basis. It retains degrees, active knots,
weights and control counts. Open sheets first clamp their active chart, matching
the public command; closed single-face shells retain their cyclic basis.
Sparse tensor solves use banded systems and a small cyclic border. No dense
control-net matrix is formed. Regular normals use Rust evaluation; singular
boundary limits use the public, open-source OpenNURBS API.

UV trims remain on their underlying chart. Rebuilt spatial boundaries require
continuous trim-image certificates within document tolerance. Solid walls use
ruled patches with certified boundary sewing. Their retained source rims use
Rhino's endpoint-weight normalization and curvature partitions. No tolerance is enlarged to accept
an inconsistent boundary. Zero-distance Loose walls contain two opposite,
coincident skins; deleting the input retains its identity.
For an already closed input, `Solid=Yes` always replaces the source with the
two-skin shell, irrespective of the hidden `DeleteInput` preference.

Loose is available only when every selected object is a single surface without
interior creases. Degree above 17, more than 262,144 stored controls, more than
16 million local basis work units, mixed-sign weights, unresolved singular
normals and uncertifiable trim/wall assembly decline before document admission.
The offset between Greville stations has no true-offset error bound. General
self-intersections, collapse policy, arbitrary singularities and comparative
performance are not qualified.

Validation uses licensed Rhino 8 command captures on private Xvfb displays and
windowless Rust command/UI replays. Authored tensor nets and trim curves construct
product inputs; captured output coefficients are expectations only. Native
unknown-option discoveries and failed attempts remain in the evidence inventory.
The retained checks compare control structure, geometry and regular normals,
boundary topology, metadata, selection, source identity, actual history and 3DM
round trips. The finite geometry gate is `1e-6`; this is not complete native parity.

[Qualification record](offset-srf-loose-validation.json) ·
[Reproduce checks](../tools/rhino_oracle/check_offset_srf_loose.py)
