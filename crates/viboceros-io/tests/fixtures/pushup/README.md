# Certified pushup edge export regression (VIB-02)

The original Rust fixture lives in
[`three_dm_curves/tests/pushup.rs`](../../../src/three_dm_curves/tests/pushup.rs).
It pushes a rational UV ellipse onto the bilinear surface `(u,v,u*v)` and a
translated lower skin, then constructs a closed three-face panel. The complete
degree-four composite curves have interior knot multiplicity five. Before the
export fix this valid solid failed with
`Native("object 0: B-rep NURBS curve is invalid in OpenNURBS")`.

Export subdivides both shared composite edges and all incident trims. The archive
has one closed solid with 3 faces, 9 edges, 8 vertices and 18 trims. The test checks
unchanged support surfaces before serialization, source immutability, readback,
and parameterized edge evaluation. Additional tests exercise reversed faces and
independent positive/negative rational gauges spanning `2^-700` through `2^700`.
The wall trims have a different parameter domain from the shared cap edges and
include reversed edge uses.

Retain a generated archive for the independent audit:

```sh
VIBOCEROS_PUSHUP_FIXTURE=/tmp/certified-pushup-panel.3dm \
  cargo test -p viboceros-io certified_ellipse_pushup_trimmed_solid_exports_without_refitting
```

[`audit.cpp`](audit.cpp) uses only public OpenNURBS APIs, without Viboceros's
reader or geometry kernel. It checks native validity, solid closure, incidence,
and all edge/trim samples against an independent rational ellipse evaluator and
surface evaluation. On Linux, set `opennurbs_build` to the `opennurbs` directory
inside a completed `target/debug/build/viboceros-io-*/out/build`, then run:

```sh
c++ -std=c++17 -O2 -Ithird_party/opennurbs \
  crates/viboceros-io/tests/fixtures/pushup/audit.cpp \
  -Wl,--start-group "$opennurbs_build/libopennurbsStatic.a" \
  "$opennurbs_build/zlib/libzlib.a" \
  "$opennurbs_build/freetype263/libopennurbs_public_freetype.a" \
  "$opennurbs_build/android_uuid/libandroid_uuid.a" -Wl,--end-group \
  -o /tmp/pushup-audit
/tmp/pushup-audit /tmp/certified-pushup-panel.3dm
```

The audit requires a maximum deviation below `1e-12`; the initial run measured
`4.78331e-16`. This fixture reproduces the reported representation failure;
it does not certify every fender construction or convert full-order support
surfaces or unrelated unsupported trim forms.
