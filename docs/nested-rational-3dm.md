# Rational storage in nested 3dm geometry

3dm import and export retain the explicit rational storage of 3D NURBS inside
polycurves and B-rep edges, including NURBS whose weights are all one. Previously
the nested Rust decoder inferred the storage from the weights, while the native
decoder always allocated rational storage. This could lose a rational flag on
import or rationalize a polynomial curve on export without changing its shape.

The bridge now carries a strict boolean after each 3D NURBS knot vector.
Polycurve payload version 3 and B-rep version 2 include it. The readers still
accept polycurve versions 1/2 and B-rep version 1, using weight inference for
those older payloads. Invalid booleans, missing flags, trailing bytes, and
polynomial flags paired with nonunit weights are rejected. The arc payload
remains version 1. These private payload versions do not change the 3dm archive
version. Surface and 2D trim allocation flags are not preserved explicitly.

`NurbsCurve::has_rational_representation()` reports storage;
`NurbsCurve::is_rational()` continues to report varying weights. Nested input
polycurves are flattened on a private copy, retaining their segment domains,
geometry, and per-leaf storage. This does not promise preservation of the
original nesting hierarchy.

Sixteen licensed Rhino 8 files cover flat and nested polycurves, a rectangular
face, and a box. Each has polynomial, rational unit-weight, mixed unit-weight,
and mixed constant-weight-seven variants. Sources use public RhinoCommon
constructors and File3dm APIs without active-document commands. Tests compare
storage flags, degrees, full knot vectors, controls/weights, domains, topology,
object and geometry text, and 33 parameter samples per 3D curve. Rhino independently
reads all sixteen Viboceros rewrites. The comparison epsilon is `1e-12` absolute
for this bounded coordinate range; flags and topology compare exactly. This is
sampled transfer evidence, not a continuous error certificate or performance
benchmark. The [validation record](nested-rational-3dm-validation.json) retains
the results and source hashes.

```sh
cargo test -p viboceros-io --lib three_dm
cargo test -p viboceros-io --test nested_rational -- --nocapture
python3 -m unittest tools.rhino_oracle.test_nested_rational
```

To regenerate with the licensed oracle:

```sh
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.generate_nested_rational
cargo build -p viboceros-io --example roundtrip_3dm
mkdir -p /tmp/viboceros-nested-rational-rewrites
for source in tools/rhino_oracle/fixtures/nested_rational/*.3dm; do
  target/debug/examples/roundtrip_3dm "$source" "/tmp/viboceros-nested-rational-rewrites/$(basename "$source")"
done
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.generate_nested_rational /tmp/viboceros-nested-rational-rewrites
```

Native source/readback JSON and fixture hashes are retained under
`tools/rhino_oracle/observations/nested_rational*`; the source archives are in
`tools/rhino_oracle/fixtures/nested_rational/`.
