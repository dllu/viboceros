# Stretch

Select objects or visible control/edit grips and run `Stretch`. Pick the axis
start and end, then enter a factor or pick the new end location. A factor can
also precede the axis end: enter the factor after the start, then pick the
direction. Enter at that prompt reuses the previous factor.

`Length` accepts a numeric new axis length, including model-unit input in the
interactive prompt. `Copy=Yes` accepts repeated placements from the original
sources. Enter finishes; Escape keeps accepted copies. One Undo restores the
command. `Rigid=Yes` places whole objects without changing their shapes and
treats each selected group as one body. Selected grips move individually.
Copy and the last accepted factor are remembered after completion or Escape;
Rigid starts at No each command.

The command API accepts complete input:

```text
Stretch 0,0,0 0,0,10 2 Copy=No Rigid=No
Stretch 0,0,0 0,0,10 Length=15
Stretch 0,0,0 0,0,10 3,4,15
Stretch 0,0,0 2 0,0,10 Copy=Yes
```

The target point contributes its signed projection along the original axis.
Let `L` be the axis length, `l` the requested new length clamped to zero,
`n` the unit axis and `t = clamp(dot(p-start,n)/L,0,1)`. The map is
`p + n*(l-L)*t²*(3-2*t)`. Points behind the start remain fixed; points beyond
the end receive a constant translation. Negative factors or lengths use the
same collapsed-end map as zero. A factor of one adds no geometry or history.
The native SDK's minimum valid axis length is strictly greater than `2^-32`.

Curves, surfaces, B-reps and meshes use the existing geometry and topology
adapters. Control/edit grip edits preserve the original interpolation basis;
curve derivative breaks become real NURBS segments. Copied grip owners leave
their source geometry intact while the source grip display retains the last
accepted displacement, including across copy Undo/Redo. Repeated copies use the
original source basis. Whole-object and grip previews use shared cached cages
in wireframe, shaded and ghosted views without changing geometry or history.

Nonlinear geometry is approximate. The command uses a minimum curve/surface
component fitting tolerance of `1e-5` model units. B-reps reserve four component
budgets for assembly and boundary validation. Larger document tolerances remain
applicable; the document setting itself is retained. The portable and optional
usd-brep interpolation producers share the point map and fitting policy.
Arbitrary large or ill-conditioned nets may exceed the bounded fitter and
return an error before changing the document. General deformations can fold
geometry; an axis stretch is not guaranteed to preserve injectivity.

Native validation includes 23 public SDK point-map cases and 119 command
workflows, replayed through both the command API and interactive prompts.
Captures cover tilted/reversed axes, lengths, point targets, factor boundaries,
grouped rigid placement, copies, selected grip owners, postselection and history.
Additional captures check remembered defaults, the fitting floor and actual
segment definitions. The [validation receipt](../stretch-validation.json)
records counts and measured differences.

Rhino's own fitted results differ from its exact point map: dense SDK probes
measure same-parameter residuals up to `6.45e-4` and forward closest-point
distances up to `3.19e-4` in the tilted fixtures. Those comparisons use a
`7e-4` bound; curve fits use `2e-5`. Viboceros fits are also checked separately
against the exact map at off-grid curve and surface stations with a `1.1e-5`
bound. These sampled checks are not continuous deviation certificates or
cross-application performance measurements.

```sh
cargo test -p viboceros-command --test stretch_command --config 'profile.test.package.viboceros-geometry.opt-level=2'
cargo test --bin viboceros stretch --config 'profile.test.package.viboceros-geometry.opt-level=2'
cargo test -p viboceros-oracle --lib stretch_points
python3 -m unittest tools.rhino_oracle.test_stretch
```

[Rhino command reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/stretch.htm)
· [Public SDK](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/T_Rhino_Geometry_Morphs_StretchSpaceMorph.htm)
