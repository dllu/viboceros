# Curved planar Inset boundaries

[Surface Inset](inset-surfaces.md) · [Meshes and SubDs](inset.md)

Planar `Inset` supports smooth closed NURBS loops, circles and circular annuli,
including narrow annuli and multiple holes. Polygon loops and smooth curved loops
can occur on the same face. Tangent-continuous line/arc chains also work.
The supporting surface and original boundaries remain unchanged.

`CreaseCorners=No` keeps a smooth offset loop as one closed edge with an annular
border. `Yes` splits a chain at its original edge joins and adds connecting strips;
a single smooth closed source edge still has an annular border. The same command,
face picker, preview, cancellation, attributes and history rules apply to all inputs.
Previews sample every nonempty curve span and share the existing segment limit.

Certified circular/linear recognition retains analytic offsets and source clocks
where possible. Other smooth curves share the planar NURBS offset fitter. Builds
with `native-smlib` supply usd-brep's existing offset, intersection and curve-edit
operations; admission and topology policy remain shared. Endpoint correspondence,
closed-loop simplicity, winding, pair contacts and nesting are checked before
staging the complete result. Narrow holes use trim-region queries rather than
requiring their control hulls to fit inside an inscribed polygon.
Convex-loop certificates use an exact convex hull; hole-pair candidates use a
cached-bound sweep to avoid scanning unrelated boundaries.

Frozen comparisons cover ellipses, periodic convex/concave splines, cubic Bézier
chains, stadium outlines, mixed polygon/circular loops, several holes, tilted
circles and narrow annuli. Analytic loci use `1e-10`. General fitted SDK curves use
`1e-6`; joined command curves allow `2e-6` for two independent fits at `1e-6` model
tolerance. The recorded ellipse command has about `9.5e-7` maximum sampled
source-distance error, while the shared fit has about `1.8e-8`. A separate regression
measures these against the original curve. Corresponding-edge comparisons run in
both directions at each curve's span stations; they do not require identical knot
tables. These remain sampled numerical fit checks, not continuous error proofs.

The first legacy command capture includes a rejected `Loose=No` token inside the
getter. Later captures use `Loose=Off`, with no unknown tokens. All successful
captures retain their exact producers and are immutable. Loose ellipse results
have different edge partitions and remain separate pending implementation.

Curved G0 corners, higher-degree polygon-edge encodings, non-affine surface charts,
seams, poles and topology-changing offsets remain pending. Offset cusps, touching
or crossing boundaries, nested holes and exhausted fitting/storage budgets return
errors without modifying the document. `Loose`, `ThroughPoint`, seam options and
separate/curve-only output modes are still unimplemented.

```sh
cargo test -p viboceros-oracle --test inset_surface
cargo test -p viboceros-oracle --features native-smlib --test inset_surface
python3 -m unittest tools.rhino_oracle.test_inset_planar_curves
```
