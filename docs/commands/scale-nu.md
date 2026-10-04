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

The origin in a complete invocation is a world-space point. `WorldCoordinates`
uses world axes for scaling. Otherwise the active CPlane supplies the directions;
its stored origin does not replace the chosen scaling origin.

Enter `ScaleNU` to select sources, then pick the origin and enter separate X, Y
and Z factors. Each factor prompt also accepts two axis reference points. Enter
accepts that axis's remembered factor, initially 1. Accepted factors survive
Cancel and Undo and belong to the command registry.

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

## Verification and limits

The [Rhino command documentation](https://docs.mcneel.com/rhino/8/help/en-us/commands/scale.htm#ScaleNU)
describes the three factor/reference phases and WorldCoordinates option.
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
the map. The cause of the native scripted behavior remains unresolved.

Rigid and SubCrv options, native mouse-preview fidelity, per-axis viewport
changes, general calculator input, all singular analytic/B-rep shapes and
performance parity are unproven or incomplete. Geometry that cannot preserve
its representation under a singular map can still reject the edit atomically.

[Capture provenance](../scale-nu-provenance.json)

```sh
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle rhino tools/rhino_oracle/fixtures/scale_nu.json --scheme VibocerosOracleScaleNU --output tools/rhino_oracle/observations/scale_nu.json --timeout 240
python3 -m unittest tools.rhino_oracle.test_scale_nu
cargo test -p viboceros scale_nu --bin viboceros
cargo test -p viboceros-command scale_nu --lib
```
