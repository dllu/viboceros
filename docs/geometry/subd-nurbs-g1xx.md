# SubD G1xx processing

[Continuity modes](subd-nurbs-continuity.md) · [Base conversion](subd-nurbs-conversion.md)

`SubDNurbsContinuity::G1xx` starts from the unpacked G0 patches. It inserts a
double knot at the first span boundary inward from each original smooth
extraordinary corner. Source component identities determine oriented face fans;
coincident coordinates do not merge vertices. Crease, corner, dart and generated
face-center vertices retain the base conversion.
Sources without smooth extraordinary vertices retain G0, including dart-only
sources.

## Odd valences

The first boundary control vectors around the retained limit point are filtered
to their first Fourier harmonic. Two accumulated vectors suffice, so this takes
linear work in the fan valence without a dense Fourier matrix.

Write `E1`, `E2`, `E3` for boundary controls inward from the corner, relative to
the corner itself. The new second control is `(6 E1 + E3) / 5`, making the first
boundary span quadratic. Its displacement also moves the adjacent interior
control, preserving the transverse derivative there. If `F_i` is the first
interior control of face `i`, compatibility requires

```text
F_i + F_(i-1) = 2 E1_i + cos(2π/n) (4 E3_i - 6 E1_i) / 15.
```

An alternating recurrence solves this cyclic system in linear work. Odd valence
has a unique solution. Coordinates relative to the retained corner avoid
subtracting large absolute positions in the solve.

## Even valences and native charts

Even fans use the same tangent filter but remove the alternating component of
the second boundary controls instead of making their first span quadratic.
Boundary corrections apply at every smooth extraordinary corner. The first
smooth or dart extraordinary corner in UV-cycle order with a smooth incoming
face edge selects the native patch degree: odd selects cubic, even selects
quartic. Dart controls remain unchanged. The incoming-edge distinction matters
on the two faces adjacent to a dart's crease.
Even-valence twists retain G0 values on cubic patches.
This can produce cubic and quartic faces on the same closed source and depends
on face-cycle rotations. The converter preserves that behavior, so it makes no
global G1 guarantee for arbitrary source charts.

At a corrected corner, exact degree elevation precedes the quartic transverse
updates. The quartic first interior controls satisfy

```text
F_i + F_(i-1) = 3 E1_i / 2 + 3 cos(2π/n) (E2_i - E1_i) / 4.
```

The cyclic solve selects zero alternating component. Boundary curves remain
shared, including mixed quad/triangle seams and different parameter scales.
Odd corrections run before elevation so adjacent odd/even corners compose.

The frozen `subd_g1xx`, `subd_g1xx_even`, `subd_g1xx_charts`,
`subd_g1xx_creases` and `subd_g1xx_degree` families contain 43 additional native
workflows. They cover valences six through nine, affine and scale controls,
triangle sectors, mixed seams, closed trapezohedra with adjacent odd/even
vertices, dart/crease chart rules, asymmetric controls and every UV corner.
Whole-image bounds use exact degree elevation and common knot refinement; spatial edges
also account for positive rational weight differences introduced by elevation.

Output admits at most four million controls, including quartic elevation.
Nonmanifold, repeated or disconnected smooth face fans and smooth valence below
three reject. [Packing](subd-nurbs-packing.md) applies after this processing;
soft sharpness and command integration remain open.
