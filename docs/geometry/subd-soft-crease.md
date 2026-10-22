# Soft SubD edge weights

[SubD geometry](../subd.md) · [Native observations](../oracle/soft-crease.md)

`SubD::try_with_soft_crease_weights` prepares immutable endpoint edits through
public OpenNURBS sharpness conversion and component setters:

```rust
let (edited, changed) = subd.try_with_soft_crease_weights(&[
    (edge_index, [20.0, 80.0]),
])?;
```

Weights are percentages in `0..=100`. Their order follows the two vertex ordinals
in `subd.edges()[edge_index]`. A weight of 100 is still soft; hard creases have
separate tags. Setting a soft weight on a hard edge updates its endpoint tags
and dependent sectors through the native tag setter. Other edges retain their
weights. Boundary edges, repeated ordinals, invalid indices, nonfinite values
and out-of-range percentages decline before changing geometry.

`edge_sharpness()` exposes the stored native values in `0..=4`, and
`edge_face_counts()` identifies interior edges. Hard creases report zero soft
sharpness. These control properties are collected once; evaluated display meshes
do not allocate the extra arrays. Repeated values return the original immutable
surface with `changed=false`. Transforms, control edits and internal archive
roundtrips retain the weights.

The [SubDCrease editor](../commands/subd-crease.md) adds staged vertex and chain
editing, viewport labels and atomic document admission. It combines this API
with the [control-chain API](subd-chains.md).

## Native archive boundaries

Rhino 8.32 changes some accepted weights when writing and rereading a 3DM.
For the captured four-by-three-by-two cube at 24.75%, the sampled surface moves
by about `0.002003` model units on native readback. External Viboceros 3DM import
matches that native readback. It is compared separately from the accepted live
command result, with the original observations retained.

The cause is repeated sanitization of float sharpness near an integer. A double
just outside the native snapping interval can round inside it when stored as a
float. Applying the snapping rule again changes that value.

Internal snapshots preserve the accepted value. The private `VIBOSUBD` envelope
remains unchanged for ordinary values. Only surfaces with affected weights use
`VIBOSUB2`: a 24-byte header records the native archive length and correction
count, followed by the native archive and 20-byte records containing a native
edge ID and its two exact float values promoted to doubles. Integers and doubles
use little-endian storage. The decoder validates counts, lengths, IDs, duplicate
records and values before returning geometry.

Restoration finds a valid input that the public sharpness factory stores as
exactly the accepted float, checks that equality, and clears evaluation caches.
It neither accesses private native storage nor changes the snapping tolerance.
Both envelopes remain bounded to 64 MiB. The envelope is private internal data;
3DM files continue to use the public native writer and reader.

## Validation

The [qualification](../subd-soft-crease-qualification.json) compares 138 source
objects across 133 native workflows, including constant and varying weights,
corners, hard edges, branches, loops, disconnected chains, multiple objects,
cancellations and no-ops. Controls, tags, vertex limit stations and density-two
surface facets agree within `1e-6`; the largest observed geometry error is about
`3.9e-15`. Eighteen precision-boundary imports separately match native archive
readback within about `1.4e-15`.

Unit regressions cover repeated snapshots at integer thresholds, oriented
endpoint weights, hard/soft conversion, source immutability, malformed
envelopes, transforms and control edits. Sampled facets do not establish a
continuous surface-error bound. The separate
[session qualification](../subd-crease-session-qualification.json) covers captured
profiles and application getters. Arbitrary profiles, full frontend equivalence
and comparative performance remain unqualified.
