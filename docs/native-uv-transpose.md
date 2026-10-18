# Exact native U/V transposition

Native surface orientation now preserves the tensor basis when it exchanges U
and V. Previously, `SmBSplineSurface::SwapUV` reconstructed the surface through
`SetCanonical`. Its knot cleanup could collapse `[1e12,1e12+1]` to a point or
merge a narrow interior span. Joining valid sheets then failed during export,
although the input had imported successfully.

The generated-source surface patch uses usd-brep's existing `N_SrfCopy` and
`N_SwapUV` on a private stack-owned tensor. `SetFromGwNurb` copies the result
before the stack is released, handles borrowed storage, notifies dependents and
rebuilds analytic parameters. The complete knot arrays and homogeneous controls
are permuted without canonical reconstruction. The patch composes with the
existing exact-copy repair; the pinned third-party checkout stays unchanged.

`SmFace::SwapUV` also formerly mirrored trim curves through a normalized
45-degree frame. Its rounded matrix mixed the coordinates of distant charts.
The face patch copies each curve and applies the existing transform with an
axis-aligned frame and Z reflection: `(u,v,z)` becomes `(v,u,z)`. The curve
parameter, knots and weights remain intact. Native loop/face orientation and the
adapter's checked source-chart restoration retain their existing roles.

Two bridge regressions check eight ordinary, translated and anisotropic charts
and two rational narrow-span cases. They require exact complete source surface
definitions after chart restoration, two faces, seven edges, one shared edge,
manifoldness and unchanged Rust/native operands. Restoration must retain the
raw native spatial vertices and edges. The narrow spans are approximately
`1e-14`; tested chart widths include `2^-40` and translated origins `1e12` and
`-2e12`. Both regressions fail with the parent kernel and pass with the patches.
Construction uses `1e-9` independently of the `0.001` modeling tolerance used
by the retained failing Join recipes. No comparison epsilon is increased.

Sixteen additional command workflows use independently authored clamped and
unclamped curved sheets over translated, unequal U/V intervals. Join/JoinCopy,
both chart orientations, reversed faces and preselection/command-first picking
check complete active coefficients, source retention, metadata, groups,
selection and two Undo/Redo cycles. The original sixteen local-chart workflows
retain their original geometry and modeling tolerance.

All 64 saved interior-isocurve Join/JoinCopy recipes now execute and produce
one connected two-face output with seven edges, including the 32 translated
cases that previously failed. Of these, only 33 have saved Rhino observations:
32 local cases and one translated case. All 33 retain strict raw differences in stored tolerances, seam positions
and representation at `1e-10` absolute and `1e-12` relative epsilon. The remaining 31 translated
cases have execution/topology evidence only. The separate 48-case curved-Join
archive executes completely, with four full raw matches and 44 differences at
`1e-10` absolute/relative epsilon. Saved observations and comparators are
unchanged.

At the transpose revision, a six-face box over translated charts returned
`orient stitched boundaries status 1006` (`SM_ERR_NULL_POINTER`) in both the
parent and candidate. The complete probe source and both failure logs remain
in that validation record. The [closed-Join orientation repair](native-closed-join.md)
subsequently lets this assembly reach the document as an outward solid. Some coincident natural
boundaries also remain unjoined: the retained wider curved-sheet probe imports
the correct active coefficients but returns eight separate edges. Native
matching at every chart scale, general trimmed/periodic topology and performance
parity remain unqualified. The patches change the parameter permutation, not
the native matching tolerances or acceptance rules.

```sh
cargo test -p viboceros-smlib --features native --test stitch_uv
cargo test -p viboceros-command --features native-smlib --test native_unclamped_join
python3 -m tools.rhino_oracle audit tools/rhino_oracle/fixtures/join_isocurve_certificates.json --native-smlib
```

The [validation record](native-uv-transpose-validation.json) retains source and
artifact hashes, actual job results, raw native audits, strict saved-observation
comparisons and the unresolved probes. Execution success is reported separately
from raw Rhino matches. No new Rhino observations or performance results are
inferred from these runs.
