# B-rep morphing

[Transforms](commands/transforms.md) · [Surface fitting](surface-morphing.md) · [Boundary validation](brep-validation.md)

`PointMorph::morph_brep` and `Brep::morphed` fit nonlinear images of trimmed
NURBS B-reps. `OrientOnSrf Rigid=No` now supports these as source objects,
without flattening them into surfaces or meshes. Its target is still an
untrimmed NURBS surface.

## Assembly and tolerance

`brep/morph` maps each shared vertex once, fits each shared edge once, and fits
each face's underlying surface. It retains native curve and surface domains,
edge endpoint indices, face reversal flags, and exact UV loops, including holes,
seams, and singular trims. Incident faces reference the same fitted edge.

Each curve/surface fit receives one quarter of the document absolute tolerance.
New vertex and edge tolerances are set to the document absolute tolerance;
looser source component tolerances are not inherited to hide a bad fit.
The assembled result must pass bidirectional model-space edge/trim checks.
Failed component fitting or assembly leaves copy/in-place document operations
unchanged, including objects, attributes, groups, selection, and undo/redo.

The entire underlying surface is fitted, including regions outside the retained
trims. The point map must be defined there. An arbitrary black-box map cannot
establish global injectivity or orientation: face reversal flags are preserved,
not automatically corrected for folds or orientation-reversing images. The
finite fit and boundary samples are not continuous error certificates, nor do
they prove absence of self-intersections. Curve and surface resource limits
still apply; valid complicated images can fail to fit.

## Shared-boundary meshing

Independently refined faces can have different boundary grids. Snapping their
existing vertices does not remove T-junctions. If ordinary smooth-seam meshing
fails its boundary/topology audit, `brep/tessellation` rebuilds a conforming
triangle mesh using one canonical sample table per shared edge. P-curve knot
corners are included, each incident trim receives corresponding model-space
samples, and constrained UV triangulation inserts the face's interior grid.
`brep/trim_image` supplies the same analytic-tangent correspondence search used
by validation. Samples must follow trim order and lie on the face within its
allowed boundary tolerance. Collapsed pole triangles are omitted; the final
mesh must still satisfy the B-rep's boundary/topology requirements.

This fallback may replace quads with triangles. Jagged-seam meshing does not
request it. The subsequent [meshing audit](brep-meshing.md) now checks open
multi-face shells too, tracing naked mesh sides to their original B-rep faces.
Ambiguous projection, degenerate samples, and triangulation/resource failures
remain explicit errors.
The fallback rejects full-order interior surface breaks, even with coincident
limits: its UV triangulation cannot safely partition positional jumps. A
regression first demonstrated the incorrect bridging of an interior crack
whose outer boundaries still validated; that case now fails tessellation.

## Validation

Native regressions cover an exact cubic box image with preserved unit volume,
a rational capped cylinder with seams and singular poles, a planar face with a
hole, shared-edge fit counts, and rejection of inherited loose tolerances.
Independent offset-grid checks cover edge parameters and entire face domains.
Both triangle and polygon meshing produce solids for the morphed cylinder;
a separate unchanged box with unequal face knots and independently varying
edge parameter speed checks the meshing correction without any morph fit.
Command tests retain B-rep type and exercise copy/in-place undo/redo.

`brep_surface_morph.json` builds four identical inputs independently in both
engines: a disk, an annulus on a warped target, and outward/inward capped faces.
The shared exact trimmed-fixture builder also serves mass-property probes.
The Rhino worker duplicates each B-rep and uses public `SplopSpaceMorph` with
explicit tolerance, `QuickPreview=false`, and `PreserveStructure=false`.
It disposes all owned sources, targets, morphs, and successful/failed results.

Topology and direct point maps are compared separately from fitted geometry.
There are 258, 363, 450, and 450 samples respectively. Vertices, face UV samples,
and lifted trims retain source correspondence. Rhino refits model-space edges
with different parameter speeds: same-parameter edge differences reached
`0.0802`, while closest-point correspondence removed that discrepancy. Therefore
edge samples compare the closest fitted point to each mapped source point using
the public [Curve.ClosestPoint API](https://developer.rhino3d.com/api/rhinocommon/rhino.geometry.curve/closestpoint).
The native probe additionally requires every original-parameter fitted edge
sample to meet the requested tolerance. The cross-engine edge check is sampled
and one-directional, not a bidirectional Hausdorff certificate.

On Rhino 8.32 under Wine/FEX, direct maps agree within `5.4e-15` per coordinate
at absolute `2e-12`, relative `1e-12`; shared topology matches exactly. Native
original-parameter errors stay below `2.0e-7` for the three `1e-6` cases and
`1.15e-6` for the `1e-5` annulus. Rhino's sampled geometric fit errors reach
`9.04e-6`, and the largest fitted-output coordinate difference is `7.99e-6`.
The fitted comparison consequently uses absolute `1e-5`, relative `1e-12`:

```sh
tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/brep_surface_morph.json \
  --absolute-epsilon 1e-5 --relative-epsilon 1e-12
```

Illustrative release fits take roughly `13–115 ms` natively and `4–34 ms` in
translated Rhino. Native fits achieve smaller measured errors, but are slower
on several cases; performance parity is not established. Startup, fixture
construction, comparison sampling, and tessellation are outside these timings.

The initial B-rep morph checkpoint passed 1,318 Rust tests, 23 Python tests,
strict Clippy, and all 118 native fixtures (1,197 operations). Recorded curve/surface
comparisons retain their documented epsilons; six fresh trimmed mass-property
comparisons pass at absolute `1e-8`, relative `1e-10` (maximum `1.22e-9`).
This is bounded regression evidence, not complete Rhino morph compatibility.

Subsequent [3DM cross-reader tests](brep-3dm-interchange.md) verify actual native
morph exports, including complete NURBS definitions and post-import meshing in
Rhino. Their serialization tolerance is independent of fitting tolerance. The
cubically lifted sphere initially exhausted the surface fitter's budget at `1e-6`.
The subsequent [rational composition candidate](surface-rational-fitting.md)
resolves that fit without increasing its sample or control limits; the current
interchange fixture requires `1e-6`.
