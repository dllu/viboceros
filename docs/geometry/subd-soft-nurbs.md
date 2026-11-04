# Soft SubD creases to NURBS

[Base conversion](subd-nurbs-conversion.md) · [Continuity](subd-nurbs-continuity.md)
· [Command](../commands/to-nurbs-subd.md)

Soft edges, chains and variable endpoint profiles support packed and unpacked
conversion. Native controls cover UI weights 10, 33.333, 50, 60, 70, 75, 80, 90
and 100 on grids, boxes, a torus, extraordinary fans and a quad/ngon prism.
All five continuity modes have native coverage, including strong soft edges at
even extraordinary vertices and shifted, rotated, affine and asymmetric fans.
Further controls cover closed loops, n-gons, mixed hard/soft chains, affine
transforms and native subdivision. Resource or topology failures remain atomic
in mixed ToNURBS commands and preserve redo, selection and preferences.

## Construction

Public OpenNURBS subdivision supplies bounded control stencils at depths 3–5.
Each endpoint's public native sharpness interval is subdivided until its adjacent
half-edge becomes smooth. Decay and integer snapping happen before midpoint
interpolation, so a maximum of the original endpoint weights is insufficient.
A source corner uses `max(3, resolution + 1)` from its face edges and the next
edge across either smooth side. Ngon sectors retain depth 3 at the face center.

Rust refines coarse cubic tensors into the finest uniform basis and copies only
controls whose complete basis support fits within two coarse spans of the corner.
Partial stencils identify unresolved sharp boundary intervals; those corners
cannot supply the local approximation. Knot cleanup inverts Boehm insertion and
accepts only complete roundoff bounds. Sharp subintervals at levels 2 and 3
retain breaks two levels finer over their one-cell halo. These breaks, the
baseline grid and local corner bands remain protected: even redundant knots
can affect the native G2 control neighborhood.

Continuity uses the same resolution counts to skip unresolved neighboring
vertices. G1x corrects only matching first source intervals. G1xx takes shared
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

Seven frozen `subd_soft_nurbs*` SDK families contain 230 workflows, each with
both packing choices. Inputs come from actual native SubDCrease edits, including
seven exact 3DM archives
from earlier native chain/label captures. All captures ran on private Xvfb.
Python checks request hashes, content-addressed producer snapshots, archive
lineage, valid native outputs and source CRC preservation.

`viboceros-io/tests/subd_soft_nurbs.rs` imports all captured source archives.
For every workflow it compares complete surface cells and stored
spatial edges at `1e-11`, plus oriented topology, trim incidence and source
immutability. Finite patch-gap bounds are reported separately from the native
image comparison. Command and
in-memory UI tests cover conversion, attributes, groups, identity and undo/redo.
General performance parity remains open.
