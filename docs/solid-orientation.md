# Conservative solid orientation

`Brep::solid_orientation()` returns `NotSolid`, `Outward`, `Inward`, or `Unknown`.
This read-only kernel query separates spatial sense from `is_closed()` (two edge
uses) and `is_solid()` (consistent oriented incidence). It is deliberately
incomplete and does not validate that a solid is embedded without intersections.
It is not yet used to normalize document insertion, replacement, or `Cap`.

## Exact support witnesses

Same-sign NURBS weights, including a common negative gauge, give a control-hull
bound. The classifier finds its minimum X and considers every shared-edge face
component tied there. A component needs a regular surface contact attaining that
bound exactly, with its tangent-plane normal parallel to X. Its oriented normal
then distinguishes inward from outward at this outside support plane. Components
whose bounds are strictly greater cannot reach this support plane.

Span endpoints, midpoints, and a trim-rectangle center propose contacts. Exact
rational arithmetic on the original binary64 coefficients verifies both contact
and normal direction. No sampling tolerance authorizes a result; the homogeneous
normal numerator avoids division and floating-point cross-product cancellation.
The same exact numerator supports [regular surface normals](surface-normals.md).

Containment is accepted only for a single exactly closed, counterclockwise
rectangle of four continuous, clamped, degree-one UV curves with same-sign
weights. Collinear multi-span sides may retrace within their segment, but gaps,
jumps, off-line controls, holes, and other trim representations are unsupported.
All tied components must supply the same sense. Mixed surface weights, an
unattained hull bound, unsupported trims, conflicting ties, or exhausting the
262,144-degree-weighted evaluation budget yield `Unknown`. This budget limits
attempted exact evaluations, not wall time or rational-integer bit complexity.

A nonzero normal **X component alone is insufficient** at a corner. For example,
the tetrahedron with vertices `(0,0,0)`, `(1,10,0)`, `(1,11,1)`, `(2,15,0)` has an
outward face normal `(15,-2,7)` at its unique minimum-X vertex. A sign-only corner
shortcut would call that face inward. Both senses of this tetrahedron remain
unresolved by the current method.

Global signed volume is also insufficient: a small outward shell farther left
than a larger inward shell can classify outward despite negative total volume.
See the [orientation audit](orientation-audit.md) for retained counterexamples.

## Shared-source Rhino comparison

The `brep_solid_orientation` oracle operation builds one native B-rep, exports an
owned 3dm artifact, verifies its native round trip, and asks Rhino's public
`Brep.SolidOrientation` getter about that same uninserted geometry. Document
insertion is intentionally excluded because it can change face sense.

The [49-case request](../tools/rhino_oracle/fixtures/solid_orientation.json) and
[raw Rhino records](../tools/rhino_oracle/observations/solid_orientation.json)
cover disconnected, nested, and coincident boxes; X/Y/Z separation, size, sense,
and source order; open and inconsistently oriented boxes; and the tetrahedron.
All 49 complete geometry records match, with zero numeric differences. Orientation
also matches in 45 cases at absolute epsilon `2e-12`, relative epsilon `1e-14`.
Four cases remain explicit compatibility gaps:

| Source | Native | Rhino 8.32 |
| --- | --- | --- |
| Coincident outward then inward shells | Unknown | Outward |
| Same shells in reversed source order | Unknown | Inward |
| Outward tetrahedron | Unknown | Outward |
| Inward tetrahedron | Unknown | Inward |

The coincident-shell answers are opposite to the earlier Rhino-constructed box
batch. Those geometric recipes have different B-rep representations; neither
capture establishes a general source-order tie rule. Both original observations
are retained unchanged. The native classifier does not guess that rule.

Native tests additionally cover rational spheres and capped cylinders, common
negative and mixed weights, work exhaustion, and unsupported but geometrically
equivalent quadratic trims. These are not additional cases in the Rhino capture.
Definition-only records retain every coefficient, knot, topology field, face
sense, and tolerance without evaluating samples that would be discarded. Shared
artifact round-trip validation still compares definitions **and** samples.
Timing fields are zero: this batch makes no performance claim.

The [provenance](solid-orientation-provenance.json) records hashes and scope.
Compaction preserved every field and IEEE-754 numeric bit pattern, including
negative zero. Reproduce the comparison or replay the retained observations:

```sh
tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/solid_orientation.json --timeout 900 \
  --absolute-epsilon 2e-12 --relative-epsilon 1e-14
python3 -m tools.rhino_oracle replay \
  tools/rhino_oracle/fixtures/solid_orientation.json \
  --observations tools/rhino_oracle/observations/solid_orientation.json \
  --absolute-epsilon 2e-12 --relative-epsilon 1e-14
cargo test -p viboceros-geometry solid_orientation
cargo test -p viboceros-oracle solid_orientation
python3 -m unittest tools.rhino_oracle.test_solid_orientation
```

Comparison and replay correctly exit nonzero for the four unresolved values.
Passing regression tests retain those gaps; they do not claim full compatibility.
General extrema, arbitrary trimmed contacts, and representation-dependent ties
remain work for a broader spatial classifier. Existing document normalization
gaps and volume-based `Cap` policy are unchanged by this query.
