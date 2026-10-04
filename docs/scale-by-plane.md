# ScaleByPlane oracle evidence

[Command usage](commands/scale-by-plane.md) · [Capture provenance](scale-by-plane-provenance.json)

Two bounded requests retain public command output from Rhino 8.32.26160.13001:

| Request | Recipes | Evidence |
| --- | ---: | --- |
| `scale_by_plane.json` | 58 | Seven plane choices; Top/Front/Perspective FromView; signed, zero, and off-plane references; rational/periodic curves, rational surfaces, meshes, lines, circles, arcs; Copy/Rigid; selected grips and parents; command-first selection |
| `scale_by_plane_boundary.json` | 48 | Small component sweeps; exact inclusive `1e-6` cutoff and its neighboring doubles; FromView with world/tilted CPlanes in three views |

Each operation starts in an empty owned document at idle. Source creation is
its own Undo record. The probe runs the public `ScaleByPlane` command, records
geometry at EndCommand and after the entire macro, then uses external Undo and
Redo. The observation files retain command histories and every command event.
Scaling points use the existing integer-mantissa formatter to round-trip
binary64 coordinates through Rhino's point parser.

For FromView, the driver sends input only to the window belonging to its owned
Rhino process on private Xvfb. It waits for a public MouseCallback acknowledging
motion in the requested viewport at the `Select Viewport` prompt before
clicking. The probe records the clicked viewport's SDK construction plane.
Response validation requires exactly one successful ScaleByPlane completion
and matching viewport acknowledgements. Failed or incomplete input cannot
produce a successful capture.

Python checks independently evaluate signed axis projections and rigid center
translations against all recorded control points or vertices. Application
tests replay each recipe both as a complete command and through incremental
prompts, with the same prescribed source geometry and point values. Geometry,
parameters, object names, selections, normal grip display, and external Undo/Redo
are compared at `1e-9`. Core tests also check atomic rejection, remembered
options, copy history, and group-center placement. Additional application tests
check retained reference points during Copy and nonmutating previews.

## Measured display limitation

`scale-by-plane-52`, `53`, and `54` combine Rigid with selected parent grips.
Their owners remain unchanged and no geometry Undo record is created. Rhino
leaves temporary grip locations from its last cursor-dependent display; Undo
removes the source creation and Redo restores the original controls. The recipe
does not prescribe that cursor. These three captures verify geometry, grip
selection, and history, while excluding grip display coordinates from positive
comparisons. Raw coordinates remain in the observations. Viboceros currently
retains the original display locations for this combination.

The [separate Object matrix](scale-by-plane-object.md) adds 64 native recipes
and 120 application replays for planar/nonplanar curves and surfaces, points,
and rejected whole-mesh ID picks.

The [curve representation matrix](scale-by-plane-curve.md) adds 64 native
recipes and 128 application replays, distinguishing supporting-plane axes of
analytic arcs from radial/tangent axes of equivalent NURBS curves.

The tests do not measure native reference-picking previews or speed. They do
not verify every Object frame, grouped layouts, trimmed B-reps, or SubCrv.
The provenance explicitly leaves full native parity unproven.

## Reproduce

```sh
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle rhino tools/rhino_oracle/fixtures/scale_by_plane.json --scheme VibocerosOracleScaleByPlane --output tools/rhino_oracle/observations/scale_by_plane.json --timeout 240
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle rhino tools/rhino_oracle/fixtures/scale_by_plane_boundary.json --scheme VibocerosOracleScaleByPlaneBoundary --output tools/rhino_oracle/observations/scale_by_plane_boundary.json --timeout 240
python3 -m unittest tools.rhino_oracle.test_scale_by_plane
cargo test --release -p viboceros-command scale_by_plane --lib
cargo test --release --bin viboceros scale_by_plane
```

The provenance hashes the retained raw files and probe sources. A new capture
has new hashes and requires review before replacing the recorded provenance.
