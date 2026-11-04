# Soft SubD creases to NURBS

[Base conversion](subd-nurbs-conversion.md) · [Continuity](subd-nurbs-continuity.md)
· [Command](../commands/to-nurbs-subd.md)

A single soft edge with equal endpoint sharpness supports packed and unpacked
conversion. Native controls cover UI weights 10, 33.333, 50, 60, 70, 75, 80, 90
and 100 on grids, boxes, a torus, extraordinary fans and a quad/ngon prism.
All five continuity modes have native coverage, including strong soft edges at
even extraordinary vertices and shifted, rotated, affine and asymmetric fans.
Multiple soft edges and variable endpoint weights also reject. Rejection is
atomic in mixed ToNURBS commands and preserves redo, selection and preferences.

## Construction

Public OpenNURBS subdivision supplies bounded control stencils at depths 3–5.
Each source corner retains its local approximation depth: the maximum endpoint
sharpness in its face edges and the next edge across either smooth side gives
`max(3, ceil(sharpness) + 1)`. A full vertex-fan maximum would incorrectly refine
unaffected corners. Ngon sectors retain depth 3 at the face center.

Rust refines coarse cubic tensors into the finest uniform basis and copies only
controls whose complete basis support fits within two coarse spans of the corner.
Partial stencils identify unresolved sharp boundary intervals; those corners
cannot supply the local approximation. Knot cleanup inverts Boehm insertion and
accepts only complete roundoff bounds. Baseline and local dyadic breaks needed
by continuity processing remain protected.

Continuity skips vertices whose local neighborhood still meets a stronger sharp
neighbor. G1x corrects only matching first source intervals. G1xx takes shared
controls from the preceding face's incoming edge use in the oriented fan;
face order and the initial edge do not select the owner. The opposite use's
third control enters the odd twist and even transverse compatibility equations.
These rules come from independent native output comparisons.

## Finite patch gaps and evidence

Rhino's finite patches can have different boundary images on incident faces,
especially after strong soft-crease G1xx processing. The converter retains those
surface and stored edge images. It records conservative edge tolerances covering
the entire edge-to-face correspondence, using a common positive control basis or
the exact rational Bernstein certificate. It does not alter document tolerance or
B-rep validation. Native zero edge-tolerance annotations are not copied blindly.
These tolerances describe the finite result; this conversion has no continuous
error certificate against the mathematical SubD limit.

Five frozen SDK families contain 175 workflows, each with both packing choices:
`subd_soft_nurbs`, `subd_soft_nurbs_holdouts`, `subd_soft_nurbs_even`,
`subd_soft_nurbs_charts` and `subd_soft_nurbs_offsets`.
Inputs come from actual native SubDCrease edits, including four exact 3DM archives
from earlier native chain/label captures. All captures ran on private Xvfb.
Python checks request hashes, content-addressed producer snapshots, archive
lineage, valid native outputs and source CRC preservation.

`viboceros-io/tests/subd_soft_nurbs.rs` imports all captured source archives.
For the 155 admitted workflows it compares complete surface cells and stored
spatial edges at `1e-11`, plus oriented topology, trim incidence and source
immutability. Maximum observed surface and spatial-edge bounds are `1.3e-13`
and `2.2e-13`, respectively. The other 20 workflows assert explicit rejection. Finite patch-gap
bounds are reported separately from the native image comparison. Command and
in-memory UI tests cover conversion, attributes, groups, identity and undo/redo.
General soft chains, variable endpoint weights and performance parity remain open.
