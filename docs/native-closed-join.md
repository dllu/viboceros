# Closed native Join orientation

Native Join and JoinCopy can close the retained six-sheet box over distant UV
charts. The parent stitched its boundaries, then returned `orient stitched
boundaries status 1006` during the ray-based normal-orientation pass. The failure
prevented the valid solid from reaching the document.

The adapter now uses usd-brep's topology-based normal unification for every
stitched assembly. The command already separates edge-connected components and
uses the existing exact spatial orientation witness to reverse a provably inward
newly closed component. That same check now supplies outward sense consistently
for closed and mixed open/closed results. It requires no mass integral. If a
wholly closed assembly contains a newly joined component with an unknown
witness, the adapter retries once with the previous native ray-orientation
path, from untouched original sources. This preserves its previous results or
errors. An unknown witness is not treated as an outward certificate. Native
geometry, matching distances, stored
tolerances, nesting rules and failure handling retain their existing roles.

`Shape::stitch` promises consistent normals, with either global sense for a
closed component. Its source-chart restoration preserves that sense. Join's
command layer chooses outward orientation. These are checked separately: bridge
cases retain signed volumes `24` or `-24`, while every closed command output has
positive volume `24`.

Eight bridge cases use U `[1e12,1e12+1]`, V `[-2e12,-2e12+2]`, both chart
orientations and four face-reversal masks. They require 12 stitched edges,
closed manifold topology, all six source faces, exact complete source surfaces,
unchanged raw native spatial edges/vertices after chart restoration, and
unchanged Rust/native operands. The source box measures `2 × 3 × 4`.

Sixty-four command workflows cover those charts and local unequal intervals,
Join/JoinCopy and preselection/command-first picking. The pick sequence has a
connected prefix so every sheet can participate. Complete source surface
coefficients and original charts match exactly. All eight vertices and all
twelve complete line-edge images match an independently authored box exactly;
positive volume is checked within `1e-9`. The workflows also check source
identity/geometry, seed metadata, groups, selection and two Undo/Redo cycles.
Construction uses `1e-9` independently of the `0.001` document tolerance.

A rotated rational sphere supplies two genuinely unknown curved-component
cases, with opposite source senses. Their complete results or errors agree with
the explicitly requested native ray path, and source geometry remains unchanged.

All 112 raw outcomes in the previous curved-Join and interior-isocurve audits
remain identical to the parent, including every stored field. Their strict
Rhino comparison differences remain recorded separately.

Both new box regressions fail with the parent bridge and pass with the candidate.
The parent command run completes the local workflows, then errors on the first
translated case; it does not supply outcomes for the remaining translated cases.
The [validation record](native-closed-join-validation.json) retains that control,
the unchanged test sources' hashes, actual candidate runs and raw replay data.

```sh
cargo test -p viboceros-smlib --features native --test stitch_remote_solid
cargo test -p viboceros-command --features native-smlib --test native_closed_join
```

The underlying kernel's ray-based surface/curve query path remains unqualified
for these translated charts. The adapter uses the existing topological operation
and exact component witness; this change does not repair every upstream query.
Some wider natural-boundary sheets still remain separate in the
[previous matching probe](native-uv-transpose.md). Arbitrary nested/self-intersecting
shells, all chart scales, raw Rhino parity and performance parity remain further
work. Saved Rhino observations and strict comparison epsilons remain unchanged.
