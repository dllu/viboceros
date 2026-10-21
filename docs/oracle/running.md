# Running oracle captures and saved replays

[Oracle overview](../oracle.md) · [Evidence index](README.md)

## Capture examples

With Rhino installed through the configured Wine/FEX launcher, run the core fixture:

```sh
tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/core.json \
  --absolute-epsilon 2e-12 --relative-epsilon 1e-12
```

To keep Wine/Rhino completely off the active desktop, use the isolated Xvfb
runner (requires `Xvfb`, `xvfb-run`, and `i3`):

```sh
tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/parabola_three_point.json \
  --absolute-epsilon 2e-12 --relative-epsilon 1e-12

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/helix.json \
  --absolute-epsilon 2e-5 --relative-epsilon 1e-12

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/spiral.json \
  --absolute-epsilon 3e-5 --relative-epsilon 1e-12

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/swept_spiral.json \
  --absolute-epsilon 2e-7 --relative-epsilon 1e-12

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/catenary.json \
  --absolute-epsilon 2e-8 --relative-epsilon 1e-12

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_through.json \
  --absolute-epsilon 2e-12 --relative-epsilon 1e-12

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_tween.json \
  --absolute-epsilon 5e-8 --relative-epsilon 1e-12

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_tween_short_samples.json \
  --absolute-epsilon 2e-12 --relative-epsilon 1e-12

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_fit.json \
  --absolute-epsilon 2e-12 --relative-epsilon 1e-12

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_rebuild.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_make_uniform.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/surface_make_uniform.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/make_uniform_commands.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_insert_knot.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/surface_insert_knot.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/insert_control_point.json

python3 -m tools.rhino_oracle replay \
  tools/rhino_oracle/fixtures/insert_control_point_protocol.json \
  --observations tools/rhino_oracle/observations/insert_control_point_protocol.json \
  --absolute-epsilon 1e-8 --relative-epsilon 1e-12

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_change_seam.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/surface_change_seam.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/reparameterize.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/reparameterize_automatic.json \
  --relative-epsilon 1e-8

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/surface_extend.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/surface_extend_length.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/surface_extend_line.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_extend_line.json \
  --absolute-epsilon 5e-14 --relative-epsilon 1e-12

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_extend_command.json \
  --absolute-epsilon 2e-8 --relative-epsilon 1e-11

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_extend_arc.json \
  --absolute-epsilon 2e-11 --relative-epsilon 2e-12

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_extend_arc_command.json \
  --absolute-epsilon 2e-8 --relative-epsilon 1e-11

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_extend_boundary_command.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/surface_shrink.json \
  --absolute-epsilon 3e-8 --relative-epsilon 1e-10

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_subcurve.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_split.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_split_command.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/surface_split_isocurve_command.json

# Direct rectangular SDK trimming; one unused outer-knot padding discrepancy
# is deliberately retained by native whole-domain no-ops.
tools/rhino_oracle/run_headless.sh rhino \
  tools/rhino_oracle/fixtures/surface_trim_geometry.json \
  --scheme VibocerosOracleSmlibBridgeFinal20261010 --timeout 240 \
  --output tools/rhino_oracle/observations/surface_trim_geometry.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/surface_split_cutting_command.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_intersect_command.json \
  --absolute-epsilon 1e-9 --relative-epsilon 1e-11

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_surface_intersect_command.json \
  --absolute-epsilon 1e-9 --relative-epsilon 1e-11

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_brep_intersect_command.json \
  --absolute-epsilon 1e-9 --relative-epsilon 1e-11

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/surface_surface_intersect_command.json \
  --absolute-epsilon 1e-9 --relative-epsilon 1e-11

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/surface_brep_intersect_command.json \
  --absolute-epsilon 1e-9 --relative-epsilon 1e-11

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/brep_brep_intersect_command.json \
  --absolute-epsilon 1e-9 --relative-epsilon 1e-11

# Full-domain curved B-rep faces against surfaces or another face.
python3 -m tools.rhino_oracle replay \
  tools/rhino_oracle/fixtures/curved_brep_face_intersect_command.json \
  --observations tools/rhino_oracle/observations/curved_brep_face_intersect_command.json

python3 -m tools.rhino_oracle replay \
  tools/rhino_oracle/fixtures/curved_brep_face_circle_intersect_command.json \
  --observations tools/rhino_oracle/observations/curved_brep_face_circle_intersect_command.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_trim_command.json \
  --absolute-epsilon 5e-6 --relative-epsilon 1e-11

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/surface_direction_edit.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/remove_knot.json \
  --absolute-epsilon 2e-11 --relative-epsilon 2e-12

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/remove_control_point.json \
  --absolute-epsilon 1e-10 --relative-epsilon 1e-10

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/remove_multi_knot.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/make_non_periodic.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_make_periodic.json \
  --absolute-epsilon 2e-11 --relative-epsilon 2e-12

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_make_periodic_degrees.json \
  --absolute-epsilon 2e-11 --relative-epsilon 2e-12

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/surface_make_periodic.json \
  --absolute-epsilon 2e-11 --relative-epsilon 2e-12

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_change_degree.json \
  --absolute-epsilon 2e-11 --relative-epsilon 2e-12

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/surface_change_degree.json \
  --absolute-epsilon 2e-11 --relative-epsilon 2e-12
```

The same workflow is importable for instrumentation and tests. Run the Python
process in the dedicated Xvfb session with
`tools/rhino_oracle/run_headless.sh exec python3 your_script.py`:

```python
from tools.rhino_oracle import OracleClient, load_request

report = OracleClient().compare(load_request("tools/rhino_oracle/fixtures/core.json"))
assert report.passed
```

Set `OracleClient(native_smlib=True)` or pass `--native-smlib` to the Python CLI
to enable the optional usd-brep backend for Viboceros probes. The selection
applies to ordinary runs, audit, saved replay and live comparison. It requires
the [native kernel setup](../../README.md); the default uses the portable build.

```python
client = OracleClient(native_smlib=True)
report = client.replay(
    load_request("tools/rhino_oracle/fixtures/blend_lines.json"),
    load_request("tools/rhino_oracle/observations/blend_lines.json"),
    absolute_epsilon=5e-8, timeout=600,
)
assert report.passed
```

Set `VIBOCEROS_RHINO_LAUNCHER` to use another launcher. McNeel's documented
`/runscript` startup interface is tried first; this project's Wine path also
has an owned-window fallback that requires `wmctrl` and `xdotool` and never
targets a pre-existing Rhino process. Set `VIBOCEROS_RHINO_UI_FALLBACK=0` to
disable it. The `viboceros` and `rhino` modes run either side independently.
On Linux the driver reports a Rhino process that exits before publishing a
response after a two-second exit grace period. If no owned Rhino process ever
appears, it reports startup failure after 30 seconds; an active process keeps
the full requested probe timeout.

## Startup diagnostics

For Wine licensing startup errors involving `AcquireCredentialsHandleW`, use
the [security DLL diagnostic](../oracle-sspi.md) to check symbol resolution and
compare native and x64 execution before running another geometry capture.
