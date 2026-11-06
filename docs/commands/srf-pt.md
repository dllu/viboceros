# SrfPt

`SrfPt` (`SurfaceFromCorners`) creates a surface from three or four corner
points. Start the command and pick or type each corner. Enter after the third
point creates a trimmed triangle; Enter at an earlier prompt cancels.

```text
SrfPt 0,0,0 8,0,0 8,5,2 0,5,2
SrfPt 0,0,0 8,0,0 2,5,3 Enter
SrfPt 0 8,0,0 2,5,3 Enter
```

Corners can lie on different construction planes. Interactive coordinates,
snapping, tracking, filters and constraints use the shared point-input pipeline.
Complete scripted coordinates are world points; a bare `0` uses the command
context's construction-plane origin. Spaced coordinate triples and calculator
components remain supported.

Four corners define a bilinear surface in perimeter order. Each UV interval
starts at zero and ends at the longest of its two opposite edge lengths.
Three corners define a parallelogram supporting surface with exact straight
triangle trims. The unused half of that supporting surface is outside the face.
The normalized `NurbsSurface::try_bilinear` geometry API remains separate.

Repeated adjacent corners are accepted when enough distinct corners remain.
Triangular construction rejects coordinate-wise coincident corner pairs at
`2^-32`, independently of document tolerance. Collinear corners can produce a
valid stored surface with zero model-space area; meshing such geometry can
report an empty result. Native geometric coincidence can also close a very
narrow chart. The command preserves its seam/singular topology in an explicit
B-rep rather than flattening away the face orientation.

Previews create no document objects. A completed surface is unselected on the
current layer and retains existing selection. Source geometry, attributes and
groups remain unchanged. One Undo removes the result; Redo restores it.
Cancellation and invalid final geometry preserve pending Redo. A document or
tolerance change invalidates an active draft.

The [validation record](../srf-pt-validation.json) compares native control nets,
knots, parameter grids, vertices, edges, trims, topology, attributes and history.
Constructed interactive outputs also round-trip through 3DM. The geometry gate
is `1e-6`; these finite comparisons are not a continuous distance certificate.
One translated, coincidence-closed, zero-volume chart retains a different native
face-orientation flag. Its geometry and topology match; this orientation case
is recorded separately and remains unqualified. General mouse trajectories,
every coordinate expression, restart behavior and comparative speed remain
unqualified.

```sh
cargo test -p viboceros-command --test srf_pt
cargo test --bin viboceros app::tests::srf_pt
python3 -m unittest tools.rhino_oracle.test_srf_pt
```

Reference: [Rhino SrfPt](https://docs.mcneel.com/rhino/8/help/en-us/commands/srfpt.htm).
