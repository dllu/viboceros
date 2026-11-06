# Tangent lines at curve boundaries

A sliding `Line Tangent` can use an open curve's endpoint when no exact tangent
reaches the requested point and its closest curve point lies at that boundary.
The start moves to the boundary and the destination projects onto its tangent.
An interior closest point with no usable tangent keeps the getter open.
A projection that produces a zero-length line also keeps the prompt open.

For a source line from `(0,0,0)` to `(10,0,0)`:

```text
Line Tangent Curve=1 5,0,0 15,4,3
```

The result runs from `(10,0,0)` to `(15,0,0)`. Use the current source object ID.
`BothSides` mirrors the projected destination about the boundary. A signed length
is applied after projection; a negative length reverses the final direction.
Planar mode first projects onto the spatial tangent, then restores the contact's
CPlane height. A deliberately fixed contact still takes precedence over Planar.

Mouse previews and commits share the same construction. Relative coordinates
use the accepted, constrained destination. Sources remain intact; completion
has one Undo/Redo entry.

The [boundary validation](../line-tangent-boundary-validation.json) adds 24 matching
native command/getter workflows, with 48 3DM round trips per build. It retains
50 native observations overall, including dense public SDK seed/subdomain sweeps.
The dataset contains 34,728 SDK queries. Successful points and tangents from
comparable recipes are checked on independently constructed curves; failed
queries retain their status. One integer-division source diagnostic is excluded.

Eight complete workflows still differ when Rhino's local search fails despite
an interior tangent. Their observations remain explicit known differences,
outside the passing construction counts. The sweeps are investigation evidence;
production code contains no sampled failure lookup table. Continuous distance,
all local-search branches, arbitrary singular curves and comparative performance
remain unqualified.

Run `python3 -m tools.rhino_oracle.check_line_tangent_boundary` for windowless
checks. See [Line Tangent](line-tangent.md) for the other getter options.
