# 3DM reference data

`rhino8_nested_polycurve.3dm` was generated through public Rhino 8 APIs by
`tools/rhino_oracle/generate_polycurve_reference.py` on 2026-09-04, using licensed
Rhino `8.32.26160.13001` in an isolated Xvfb session. It contains one nested line–quarter-circle–line
polycurve, not a tessellation or proprietary implementation data.

The importer test independently checks segment endpoints, the analytic total
length, the rational arc's degree/weights and circular locus, object metadata,
and a subsequent OpenNURBS round trip. Interior angular and rational arc
parameters need not coincide.

To regenerate, run a copy of the generator in an empty temporary directory
inside an isolated Rhino 8 instance. It writes the reference and a version/result
JSON beside itself and exits Rhino. Inspect that JSON before replacing this fixture.

## Subnormal-weight import regression

`pre_scale_subnormal_weights.3dm` was written by Viboceros commit `0b08c71`
using the adjacent JSON request on 2026-09-04. This is an OpenNURBS-generated
regression fixture, **not a Rhino reference**. It contains four copies of one
quadratic with weights `1e-320`, `5e-321`, `1e-320` and all visibility/locking
combinations. Its Euclidean controls are exactly `(1,1,0)`, `(2,3,1)`, `(3,1,0)`.

That producer wrote the file successfully but its import step reported
`unsupported interchange object`: OpenNURBS' public Euclidean getter first
forms `1/weight`, which overflows. Tests require direct homogeneous division
to recover the original controls and weights, then verify a safe rewritten file.
New exports normalize this case, so regenerating with current code would remove
the input condition this fixture protects.

To reproduce in a separate worktree at `0b08c71`, copy the adjacent JSON request
there and run `cargo run --release -p viboceros-oracle -- REQUEST RESPONSE` from
that worktree's root. The request writes to the fixture path; it must not already
exist. The response's import error is expected for that old producer.

SHA-256: `115a1a6cd7fddd79550bf39d99afad2f8e75f8ed79d6491139ca0142c7f77ba1`.
