# Exact planar solid orientation

The planar path in `brep/solid_orientation/planar` broadens the
[solid-orientation query](solid-orientation.md) beyond axis-aligned support
contacts. It classifies valid embedded planar shells by exact outside crossings,
not by a corner normal component or summed signed volume.

## Geometry and predicates

`face` extracts two exact image classes from clamped, equal-weight 2×2 bilinear
surfaces. Affine patches map continuous degree-one UV trim spans to exact spatial
segments, including rational trim weights of a common sign, multiple spans,
concave loops, and holes. Non-affine but convex planar bilinear rectangles map
to their corner polygons, including a collapsed side representing a triangle.
Planarity, convexity, UV containment, and boundary closure are exact predicates.
Near-planarity at modeling tolerance is not accepted as exact planarity.

Polygon extrema select the globally minimum-X shared-edge components. This uses
the actual trimmed faces: a triangular face's unused parallelogram extension
must not select a different shell. Every tied component must classify with the
same sense; conflicting coincident shells still return `Unknown`.

`ray` proposes exact rational Y/Z stations from face-vertex combinations and
intersects the whole selected component with a +X line. Plane intersections and
projected winding are evaluated with rational arithmetic on the stored binary64
coefficients. The first regular interior crossing from minus infinity determines
sense. Holes are excluded. Edge contacts, coplanar lines, tied first hits, and
exhausting the bounded proposal/predicate work remain unresolved; numeric jitter
or a tolerance does not choose a side. Geometry and its parameterization are not
modified, and no floating-point ray origin at an enormous distance is needed.

This does not validate global non-self-intersection or support arbitrary planar
NURBS representations. Unsupported images retain the conservative curved-support
path. It does not change document insertion, replacement, `Flip`, or `Cap` policy.

## Shared-source evidence

The [68-case request](../tools/rhino_oracle/fixtures/solid_orientation_polyhedra.json)
and [raw observations](../tools/rhino_oracle/observations/solid_orientation_polyhedra.json)
cover tetrahedra, boxes, concave prisms, and hollow square tubes. Variants include
an integer shear of determinant 42, reflection of determinant −42, translation
by `(2^40, −2^40, 2^40)`, global face reversal, and reversed face-table order.
Four disconnected-shell cases distinguish actual trim extrema from untrimmed
control bounds. All queries read the same native-exported 3dm without insertion.

All 68 geometry records agree exactly: zero numeric differences in every
coefficient, knot, topology field, face sense, and tolerance. Native orientation
agrees with independent exact geometric witnesses in all 68 cases; 58 also
match the public Rhino 8.32 `SolidOrientation` getter. Ten getter differences
occur only among the 16 translated cases:

| Shell | Translated variants | Orientation differences |
| --- | ---: | ---: |
| Tetrahedron | 4 | 4 |
| Box | 4 | 2 (reversed face-table order) |
| Concave prism | 4 | 0 |
| Hollow square tube | 4 | 4 |

Tests verify that **every** recorded translated geometry is identical to its
centered counterpart after exact inverse translation of all spatial coordinates.
UV trims, knots, domains, topology, face senses, and tolerances are unchanged.
Independent `Fraction`-based polyhedral integrals verify source orientation:
untransformed volumes are `5/6`, `8`, `14`, and `24`, respectively; the linear
map multiplies these by its determinant, while translation cannot change them.
These integrals are independent source checks, not the classifier's algorithm.

A fresh owned session repeats [eight unchanged sources](../tools/rhino_oracle/fixtures/solid_orientation_translation_repeat.json).
Its [complete records](../tools/rhino_oracle/observations/solid_orientation_translation_repeat.json)
reproduce all eight original results, including three translated discrepancies
and an unchanged concave-prism control. This is evidence of translation-sensitive
public getter results for these inputs, not proof of Rhino's internal cause.
No proprietary implementation was inspected. The geometric classifier remains
translation invariant; the ten differences remain explicit compatibility gaps,
not rewritten observations or passing comparisons.

The original 49-case batch now has 47 matches, resolving both corner-only cases;
the two coincident opposing-shell cases remain unresolved. Native tests also
cover real affine faces with holes, both triangle parameterizations, nearly planar
rejection, positive/negative rational polyline trims, exact closure, work limits,
and ray edge contacts. They do not establish completeness for all geometry.

The [provenance](planar-orientation-provenance.json) records both sessions, hashes,
and comparison counts. JSON compaction preserved every value, including numeric
IEEE-754 bit patterns and negative zero. Query timings are deliberately zero;
this is not a performance benchmark.

```sh
tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/solid_orientation_polyhedra.json --timeout 1200 \
  --absolute-epsilon 2e-12 --relative-epsilon 1e-14
python3 -m tools.rhino_oracle replay \
  tools/rhino_oracle/fixtures/solid_orientation_polyhedra.json \
  --observations tools/rhino_oracle/observations/solid_orientation_polyhedra.json \
  --absolute-epsilon 2e-12 --relative-epsilon 1e-14
cargo test -p viboceros-geometry solid_orientation
cargo test -p viboceros-oracle solid_orientation
python3 -m unittest tools.rhino_oracle.test_solid_orientation_polyhedra
```

Comparison/replay correctly exit nonzero for the ten recorded differences.
Regression tests explicitly preserve that distinction from geometric correctness.
