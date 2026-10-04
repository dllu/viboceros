# ScaleNU

[Command reference](README.md) · [Transforms](transforms.md)

`ScaleNU` scales selected objects or displayed selected grips independently along
the active construction plane's X, Y and Z axes. A selected parent's picked grips
take precedence over transforming the whole parent. Signed factors mirror an
axis; zero flattens it. Mesh edits retain face indices, colors and n-gons even
when a face collapses.

```text
ScaleNU 1,2,3 2 -1 .5
ScaleNU 0,0,0 0 1 1 WorldCoordinates
ScaleNU 0,0,0 2,0,0 6 6,0,0 1 1 Copy=Yes
```

The origin in a complete invocation is a world-space point.
`WorldCoordinates=Yes` uses world axes for scaling; `WorldCoordinates=No` uses
the active CPlane directions. A bare `WorldCoordinates` toggles the choice.
It starts at No each command and can change before choosing the origin.
The CPlane's stored origin does not replace the chosen scaling origin.

Enter `ScaleNU` to select sources, then pick the origin and enter separate X, Y
and Z factors. Each factor prompt also accepts two axis reference points. Enter
accepts that axis's remembered factor, initially 1. The defaults update together
after a complete X/Y/Z set is accepted. Canceling after only X or Y retains the
previous complete set. Defaults survive Undo and belong to the command registry.

At a second reference prompt, a number sets the target axis distance and awaits
a point. For example, the third invocation above establishes an X reference
distance of 2, constrains the new distance to 6, then confirms a target point:
X scales by 3. Point references measure unsigned axis distances; use a numeric
factor for reflection. Typed interactive coordinates use normal CPlane input
rules; prefix `w` for world coordinates.

`Copy=Yes` creates a result after Z and returns to the X prompt with the same
origin and sources. Enter at that new X prompt finishes; Enter during Y or Z
accepts its default. Accepted copies share one Undo entry. Copying grips duplicates
their entire owner, changing only the picked controls. Original and copied grip
picks remain displayed. Numeric factors are staged until all three are accepted.
Reference and partial-factor previews share the command's affine map.

## Rigid placement

`Rigid=Yes` translates each object so its tight bounding-box center follows the
scale map, preserving its shape. Selected members of a group share that group's
combined tight bounds. Overlapping membership contributes to every selected
group's bounds; each object uses its last membership for placement. Groups are
processed in first encounter order, which also determines result order.

`Rigid=No` restores ordinary scaling; a bare `Rigid` toggles the choice. Rigid
can change at the origin or factor prompts and is remembered immediately,
including after cancellation. It starts at No in a fresh application session.
Previews cache centers and reuse source display geometry, applying individual
translations to objects and their clipping bounds.

Rigid excludes picked grip owners, even when their parents are also selected.
With Copy=Yes their geometry and displayed grips remain unchanged, and ordinary
selected peers can still be copied. In the captured Copy=No numeric workflows,
Rhino leaves selected grips displayed under the partial X/Y map, with Z factor 1,
while owner geometry remains unchanged. Viboceros retains these temporary display
positions without creating a geometry Undo step. A following Move uses the
original controls; Undo/Redo restores their display.

## Verification and limits

The [Rhino command documentation](https://docs.mcneel.com/rhino/8/help/en-us/commands/scale.htm#ScaleNU)
describes the factor/reference phases, WorldCoordinates and Rigid options.
The retained capture has 26 public command recipes in an empty owned Rhino 8
document on private Xvfb. Twenty workflows match complete geometry definitions,
object order, grip display, selection, Undo and Redo within `1e-9`; application
tests exercise each twice. Sources include points, rational and periodic curves,
rational surfaces and meshes.

Six scripted reference recipes remain incompatible: `scale-nu-5`, `10`, `11`,
`12`, `20` and `22`. Native bare coordinate targets yield different scales from
their axis-distance ratios, and Top/Perspective differ. These raw outputs,
command histories and camera settings are retained. Distance-constrained
reference recipes match. Viboceros's unconstrained typed targets use their
projected axis coordinates; no native discrepancy is silently substituted into
the map.

### Controlled cursor and keyboard input

A [second capture](../scale-nu-reference-provenance.json) records 32 prescribed
inputs, including complete camera settings, integer cursor positions, viewing
rays, prompt phases, geometry, command completion and Undo/Redo. Twenty real
mouse and keyboard workflows match application replay within `1e-9`. They
exercise all three axes, Top/Front/Perspective views, positive and negative
cursor locations, off-axis references and cursor positions away from typed targets.

Twelve additional recipes calibrate a cursor point through the public `GetPoint`
API, then run complete `ScaleNU` macros with a different coordinate target. In
these captures the resulting scale follows the preceding cursor calibration,
despite the target appearing in command history. For example, with an X reference
of 2 and a target of 6, a cursor coordinate of `3.9948849104859336` produces that
X coordinate on a source at `[2,3,4]`. Typing the same target at the reference
prompt produces `[6,3,4]` at either tested cursor location. This establishes the
observed input-context dependency; the native implementation was not inspected.
These twelve macros remain diagnostics, not passing application comparisons.

The original six discrepancies remain recorded. Their captures did not include
cursor coordinates, so the new evidence cannot reconstruct each original input
context. Viboceros does not currently emulate this native scripted behavior.

### Options, groups and defaults

The [option capture](../scale-nu-options-provenance.json) adds 36 complete public
command recipes and 72 application replays. It covers Boolean toggles, rotated
and tilted CPlanes, all nine source kinds, tight versus control bounds, overlapping
groups, Copy loops, grip exclusion and preferences after cancellation. Complete
geometry definitions, ordering, group membership, selections, displayed grips and
external Undo/Redo agree within `1e-9`. Three additional captured follow-up Move
workflows replay twice each and confirm that temporary grip positions never
become the controls used by the next edit.

SubCrv options, native preview appearance, live Rigid grip picking, per-axis viewport
changes, general calculator input, all singular analytic/B-rep shapes and
performance parity are unproven or incomplete. Geometry that cannot preserve
its representation under a singular map can still reject the edit atomically.

[Capture provenance](../scale-nu-provenance.json)

```sh
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle rhino tools/rhino_oracle/fixtures/scale_nu.json --scheme VibocerosOracleScaleNU --output tools/rhino_oracle/observations/scale_nu.json --timeout 240
python3 -m unittest tools.rhino_oracle.test_scale_nu
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle rhino tools/rhino_oracle/fixtures/scale_nu_reference.json --scheme VibocerosOracleScaleNUReference --output tools/rhino_oracle/observations/scale_nu_reference.json --timeout 300
python3 -m unittest tools.rhino_oracle.test_scale_nu_reference
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle rhino tools/rhino_oracle/fixtures/scale_nu_options.json --scheme VibocerosOracleScaleNUOptions --output tools/rhino_oracle/observations/scale_nu_options.json --timeout 240
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle rhino tools/rhino_oracle/fixtures/scale_nu_pending_grips.json --scheme VibocerosOracleScaleNUOptions --output tools/rhino_oracle/observations/scale_nu_pending_grips.json --timeout 180
python3 -m unittest tools.rhino_oracle.test_scale_nu_options
cargo test -p viboceros scale_nu --bin viboceros
cargo test -p viboceros-command scale_nu --lib
```
