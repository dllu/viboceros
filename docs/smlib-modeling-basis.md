# Exact native modeling bases

Native standalone extrusion, apex, translational sweep and revolution now retain
the imported curve bases through kernel copies and surface construction. The
geometry remains produced by existing usd-brep constructors. A shared C++ basis
adapter compares full knot data and uses existing `SetExpert` to reload the
native homogeneous controls when constructor knot fixup changes the basis.
Restoration rejects a changed control layout or nonfinite data, and runs only on
independently owned geometry under the existing kernel lock.

The shared adapter also replaces the importer-specific restoration code. Using
homogeneous getters avoids a Euclidean divide/multiply round trip while moving
the native controls. Surface getters and setters use V as the inner storage
dimension; the pinned getter header's stride description is reversed relative
to its implementation. Planar and capped linear-sweep input copies share the
same curve-copy preservation helper.

Two producers require an additional parameter-chart choice. Revolution's
`GetCanonical` and extrusion's `SwapUV` use tolerance-grouped unique knots.
For close spans, a private curve uses an exact power-of-two expansion before
these calls, separating distinct groups above the absolute query tolerance.
The existing constructors still compute every surface control. Their output
bases are checked before and after chart changes and restored to the supplied
profile knots. Ordinary inputs borrow their native profile without an extra
working copy. A working chart that overflows finite knot values is rejected.

Independent regressions cover all four sheet producers with a rational profile
containing a ten-unit control bulge over a `1e-14` parameter span, at chart scales
`2^-360`, `1` and `2^360`. Targeted queries include the narrow-span midpoint.
Extrusion/apex/path sweeps compare against affine point formulas; revolution
checks rotated endpoints, radial distance and height. Tests also retain input
handle evaluation and exercise both axes in the translational tensor.

These fixtures qualify standalone sheets. General tiny-chart capped topology,
arbitrary angular degeneracy and all downstream kernel copies remain open.
The changes preserve document parsing, selection, attributes and history through
the existing command adapters. See the [limitations ledger](usd-brep-limitations.md)
for source audits and remaining kernel restrictions.

The [new live fixture](../tools/rhino_oracle/fixtures/curve_modeling_short_spans.json)
and [Rhino observations](../tools/rhino_oracle/observations/curve_modeling_short_spans.json)
retain all four command records. Apex and path extrusion match complete raw
definitions. The original extrusion comparison exposed an application direction
gap for its XZ profile. The [numeric-direction integration](smlib-extrusion-direction.md)
now extrudes along positive Y and matches the complete raw extrusion record.
Revolution has matching controls, weights and full-multiplicity knot
groups, but Rhino rescales V to `10.837476096278881`. The secondary Bezier-image
signature therefore agrees while the raw parameter difference remains. These
are application compatibility differences, not established usd-brep geometry bugs.

The [comparison](smlib-modeling-basis-comparison.json) also replays 23 retained
observations without execution failures, with the same 22 raw matches as before.
The existing unclamped-path difference remains and its seven sampled geometry
grids match. New observations were captured successfully using the previously
validated oracle profile after an isolated new-profile launch stopped at Licensing.
Native audit times are placeholders and do not establish kernel speedups.

Release verification passed 1,310 command tests and 68 native bridge tests
(three ignored), native clippy with warnings denied and workspace formatting.

```sh
CMAKE_BUILD_PARALLEL_LEVEL=10 cargo test --release -p viboceros-smlib \
  --features native --test modeling_basis --test curve_import
```
