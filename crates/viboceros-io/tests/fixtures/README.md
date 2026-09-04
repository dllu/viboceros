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
