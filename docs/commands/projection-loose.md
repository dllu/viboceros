# Loose Project and Pull

`Project Loose=Yes` projects a curve's Greville edit points in the selected
direction. `Pull Loose=Yes` pulls those points toward the target. The result is
a NURBS curve with the original degree, knots, weights and control count.
Edit points constrain only their own parameter locations: the interpolated
curve may leave the target or cross trim holes between them.

```text
Project Sources=curve-id Targets=surface-id Loose=Yes Direction=0,0,-1
Pull Sources=curve-id Targets=mesh-id Loose=Yes DeleteInput=Yes
```

Both builds support the mode through existing point queries and interpolation.
The app remembers Loose independently for Project and Pull. Source/target
picking, custom directions, temporary subcurves, infinite planes, output layers,
DeleteInput and one Undo/Redo edit use the shared coordinator. Infinite-plane
projection remains an exact affine image. Point objects keep ordinary point
behavior. An entirely missed operation preserves its inputs and creates no edit.

Finite inputs support clamped non-periodic curves and periodic curves with
exactly repeated tail controls, common-sign weights, degree through 17 and up
to 1,000 stored controls. Periodic stations use the existing domain-aligned
Greville convention and omit repeated tails; reconstruction restores those
controls. Full-order knots retain separate left/right edit points. Unclamped
non-periodic curves, mixed-sign weights, degenerate stations and exhausted
arithmetic or precision bounds decline. These are bounds, not a guarantee that
every system within them is numerically solvable.

The portable path reuses faer collocation. The native path reuses usd-brep's
fixed-row and nonuniform periodic interpolation. Only the solve is delegated;
the geometry core retains weights and reconstructs original controls. Exact
rational basis evaluation bounds the reconstructed station residual against
the requested stored target positions at absolute model tolerance. A failed or
inaccurate backend cannot admit a curve merely because rounded evaluation hides
its residual. Source point evaluation and target queries retain their existing
numerical policies; this is not a whole-curve surface-image certificate.

Project keeps complete ordered hit branches whose facing agrees at every edit
point. Changed hit counts, tangent/seam ambiguities and facing changes decline.
Planar hits reuse the shared affine plane transform and original trim checks;
curved hits use the existing point intersector.
This is a conservative discrete policy; global branch association and wrapping
between edit points remain unqualified. Surface Pull requires valid normal feet
inside face trims. Mesh Pull reuses closest points, including mesh boundaries.
Project mesh points use closed represented triangles: a boundary-adjacent
Greville point rounded outside can miss. That case is retained in the fixture;
Rhino's boundary-tolerance behavior remains unqualified.

The four Python curve projection/pull operations accept an optional `loose`
boolean, default false. Loose results additionally retain full NURBS definitions
for structural comparisons. The worker uses public Rhino Loose SDK overloads
(Rhino 8.17 or newer), retaining old overloads when false. Tests cover overload
selection, output recording and source/result/target disposal on failures.
Fresh Rhino captures remain blocked by the existing [Wine SSPI issue](../oracle-sspi.md).

```sh
cargo test -p viboceros-geometry --lib curve_edit_points
cargo test -p viboceros-command --lib project_loose
python3 -m tools.rhino_oracle viboceros \
  tools/rhino_oracle/fixtures/projection_loose.json --native-smlib
```

See [qualification](../projection-loose-qualification.json), and Rhino's
[Project](https://docs.mcneel.com/rhino/8/help/en-us/commands/project.htm) and
[Pull](https://docs.mcneel.com/rhino/8/help/en-us/commands/pull.htm) descriptions.
