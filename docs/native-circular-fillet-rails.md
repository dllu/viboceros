# Circular fillet rail correspondence

The circular cross-section generator now keeps its traced rails and uses
usd-brep's existing rail-constrained skinning routine for curved fillets.
The old revolution shortcut recognized the circular loci, built a fresh
rational revolution, and affinely mapped its domain onto the rail interval.
That did not preserve the rails' parameterization or their parent UV curves.
Matching parameter values on a quarter-cylinder fillet could differ by
`0.0957` model units. Trimming the cylinder rail then left a cap gap around
`0.00177`, and face construction correctly rejected that gap.

The repair removes that shortcut from `SmCircularCrossSectionFSG`; parallel
linear rails still use the ruled-surface branch. It preserves the existing
G2 generator repair, native tolerances, source preparation and extension policy.
The upstream checkout stays immutable: CMake applies a separate patch after
the existing G2 patch and compiles the generated translation unit.

The direct native regression constructs exact rational circular centerlines
and rails independently, including Möbius changes of parameter, reversed
directions, shifted domains, scales and rigid placements. It checks the surface
boundary and rail at 65 matching parameters, checks the original rail against
homogeneous Bernstein evaluation, and checks the surface against the torus
equation at stations on every output span after deleting its input curves.
These checks distinguish shared
parameterization from coincident geometric loci. The baseline executable
overrides only the pre-repair generator translation unit.

The baseline fails 144 of the 216 cases; the repair passes all 216 normally
and under AddressSanitizer. The largest observed same-parameter rail gap in
the normal repaired run is `3.18e-14`. The existing 56 fillet geometry/lifetime
cases and 10 invalid-input cases also pass in both builds. ASan leak detection
is disabled.

The loft may have more control points than the rejected revolution shortcut.
For the traced quarter-cylinder example, the underlying surface grows from
3 by 3 to roughly 322 by 3 control points. A future compact representation
must preserve the rail and parent UV correspondence together. This change
does not establish a general throughput bound.

Under an identical experimental smooth-continuation policy, the controlled
baseline constructs 2 of 14 capped faces and the repair constructs all 14.
That prototype is separate from the production extension
policy. Curved `Extend=Yes` endpoint agreement with Rhino and the FilletSrf
command workflow remain unfinished. See the
[validation record](native-circular-fillet-rails-validation.json) for retained
results and their scope.

```sh
cargo test --release -p viboceros-smlib --features native --test circular_fillet_rails --test surface_fillets
```
