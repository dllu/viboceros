# Edge surfaces

[Surface commands](commands/surfaces.md) · [Loft](loft.md) · [Oracle](oracle.md)

`EdgeSrf` creates one B-rep face from two, three, or four selected open curves.
It retains input objects, attributes, groups, and selection. The unselected
output has fresh attributes on the current layer and no groups. Creation is
one atomic undo/redo operation; invalid selections or unsupported arguments
leave the document unchanged.

Two curves form a ruled surface; the first curve sets the V direction and the
second is reversed when that better matches their endpoints. Three curves form
a triangular patch: a collapsed side lies opposite the first boundary of the
ordered loop. Four curves form a quadrilateral Coons patch. The first boundary
of a four-curve loop runs along V; its next neighbor runs along U.
These constructions follow [Rhino's EdgeSrf command](https://docs.mcneel.com/rhino/8/help/en-us/commands/edgesrf.htm)
and public [Brep.CreateEdgeSurface API](https://developer.rhino3d.com/api/rhinocommon/rhino.geometry.brep/createedgesurface).

## Construction

`edge_surface/arrange` searches the bounded set of curve permutations and
directions. It retains the first input's direction, then places the largest
endpoint gap at the loop closure for disconnected boundaries. Existing closed
loops retain their first boundary. This ordering matches the tested reversal,
permutation and single-gap cases; arbitrary ambiguous disconnected arrangements
are not exhaustively covered.

Three- and four-curve boundaries undergo endpoint-weight normalization before
degree/knot matching. This Möbius reparameterization preserves their geometric
images. A pair uses the longer native parameter domain, with the first curve
winning ties. Matching elevates degrees and inserts knots; near-coincident next
knots from opposite curves coalesce to the smaller value using the OpenNURBS
pair-matching tolerance. Distinct nearby knots within one source are not merged.
Knot coalescing can slightly perturb a boundary, so exact input-image preservation
is not unconditional. Two-curve rulings retain the source homogeneous weight
scales and do not perform endpoint-weight normalization.

Endpoint gaps are closed by averaging the neighboring endpoints and adding an
affine homogeneous correction along each curve. **This can move the supplied
boundaries**, including interior controls, even for large gaps. It is not a
closest-fit or tolerance-constrained correction. The rational correction is not
the same as linearly moving Euclidean control points.

`edge_surface/coons` blends the compatible homogeneous boundaries, subtracting
their bilinear corner contribution. Linear functions are represented by Greville
coefficients in each NURBS basis. Boundary controls are retained directly;
interior calculations use a local origin to avoid unnecessary world-offset
cancellation. The resulting tensor surface passes the existing shared-topology
B-rep constructor and boundary validator, including singular triangular sides.

## Controls at infinity

A finite rational patch can have a zero-weight homogeneous control with a
nonzero XYZ contribution. Discarding that contribution would change the surface.
The current kernel stores finite Euclidean control points with nonzero weights,
so the Coons constructor instead elevates both U boundaries exactly and recomputes
the blend. Homogeneous Coons blending commutes with degree elevation: this changes
the basis, not the geometric surface. Up to four such elevations are attempted
within the control budget; failure is explicit if a finite-control representation
has not been obtained. This is not support for arbitrary imported controls at
infinity elsewhere in the kernel.

The oracle's zero-weight case requests `comparison_degree: [3, 2]`. Both engines
record the same exactly elevated basis, checking full coefficients rather than
dropping structural checks. Separately, each engine records a 13×13 grid on its
original surface and verifies that the comparison elevation changes none of those
points beyond `2e-12 + 1e-14 * max(|a|, |b|)`. Native unit tests compare against
independent homogeneous Bernstein blending, including negative interior weights
and the zero-weight-control case. This representation normalization is distinct
from approximate surface fitting.

## Tests and limits

Permanent fixtures contain 33 API cases and eight command cases: lines,
quadratics, rational and mixed-degree boundaries, unequal endpoint weights,
different knots and domains, reversals, reordered boundaries, two-curve rulings,
triangular singularities, and endpoint gaps. Complete degrees, control nets,
weights, knots, domains, direct samples, validity, face orientation and topology
counts are compared. Command cases also compare document state.
The live Rhino 8.32 comparison passes all 41 at absolute `2e-12`, relative
`1e-14`, with maximum numeric difference `7.33e-15`.

Four [same-file 3DM cases](brep-3dm-interchange.md) additionally cover rational
quadrilaterals, the elevated projective-control case, a rational triangle, and
shifted native domains. They check serialized topology and coefficients, lifted
trim samples, and valid manifold meshing through both readers.
A combined fresh headless run passes all 90 operations at these same tolerances:
the 41 API/command cases, four EdgeSrf 3DM cases, 33 Loft command regressions,
four Loft 3DM cases, and eight other constructed/morphed B-rep 3DM cases.

```sh
tools/rhino_oracle/run_headless.sh compare tools/rhino_oracle/fixtures/edge_surface.json \
  --absolute-epsilon 2e-12 --relative-epsilon 1e-14
tools/rhino_oracle/run_headless.sh compare tools/rhino_oracle/fixtures/edge_surface_command.json \
  --absolute-epsilon 2e-12 --relative-epsilon 1e-14
tools/rhino_oracle/run_headless.sh compare tools/rhino_oracle/fixtures/edge_surface_3dm_interchange.json \
  --absolute-epsilon 2e-12 --relative-epsilon 1e-14
```

Each compatible direction is limited to 512 controls. Full-order positional
breaks and topologically closed input curves are rejected. No general pole,
self-intersection, or continuous-error certificate is claimed. Subobject edge
selection, interactive subcurve extraction, and arbitrary curve-network surfaces
are not implemented by this command. It does not cap, fit, rebuild, or split the
face at creases. API timing covers repeated construction; command timing includes
document setup, execution, recording and cleanup.
