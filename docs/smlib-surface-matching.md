# NURBS surface-edge matching

The portable `try_match_nurbs_surfaces` and native
`viboceros_smlib::match_surface_supports` APIs implement one-sided matching of
open, untrimmed, positive-weight NURBS supports. The [MatchSrf command](commands/match-srf.md) adds original boundary
picks, readonly previews, remembered options and atomic acceptance.

Choose a natural boundary on each support, continuity (position, tangency or
curvature), opposite-end preservation and isocurve alignment. The source's
transverse sites follow its increasing parameter. Target boundary traversal
follows the selected target side; `reverse_match` toggles it. Closest-point
matching computes independent target contacts instead of affine traversal.

The implementation evaluates boundary jets at Greville sites, then solves for
homogeneous control rows. It retains boundary and first-row weight functions.
Curvature matching resets the second weight after extracting each isocurve.
The native path delegates clamping, degree changes, knot insertion and batched
interpolation to SMLib; both paths share the geometry policy.

Short supports gain enough controls to separate matched and preserved ends.
Single-span degree elevation is exact. Short multispans use the measured
Greville rebuild with unchanged interior knot multiplicities, which can change
the original surface image before matching. Nonuniform cross-boundary knots
also affect the native curvature rule. Opposite-end preservation is therefore
not a general certificate of unchanged derivatives.

Refinement rematches the evolving surface. Its fourth knot insertion returns
without another match, following the measured SDK limit. Native captures at
requested angles of `1e-5`, `1e-6` and `1e-7` radians contain a maximum sampled
normal mismatch of about `2.39e-5` radians. A successful return does not certify
that all requested continuity tolerances were achieved.

[Finite validation](surface-match-validation.json) covers authored polynomial
and rational inputs, different degrees, nonunit domains, all natural sides,
isocurve modes, closest contacts, refinement, topology and 3DM round trips.
Native output controls are expectations only. Six averaging observations are
retained for future implementation. One supplemental capture lacks its launcher
exit receipt; all its recipes were repeated in an independent owned launch.

Averaging, curve targets, trimmed and closed boundaries, broader refinement
parameter sweeps, continuous surface-distance bounds and performance parity
remain unqualified or pending. See the public
[matching API](https://developer.rhino3d.com/api/rhinocommon/type/rhino.geometry.brep/)
and [settings](https://developer.rhino3d.com/api/rhinocommon/type/rhino.geometry.matchsrfsettings/).

```sh
python3 -m tools.rhino_oracle.check_surface_match
python3 -m unittest tools.rhino_oracle.test_surface_match_record
```
