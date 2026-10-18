# Remap objects between construction planes

[Command reference](README.md) · [Project overview](../../README.md)

`RemapCPlane` moves and rotates objects so their coordinates relative to the
source construction plane become the same coordinates relative to the target.
It preserves size and height above the plane. This follows
[McNeel's command workflow](https://docs.mcneel.com/rhino/8/help/en-us/commands/remapcplane.htm).

Select objects or control grips and start `RemapCPlane`, then click the target
viewport. Without preselection, pick sources and press Enter first. The source
plane is captured when source selection finishes; choosing another viewport or
transparently editing a CPlane afterward does not replace it. Remapping changes
objects without changing either viewport's camera or construction plane.

At the target prompt:

- `Copy=Yes|No` selects copies or replacements. The initial default is No;
  successful choices follow `RememberCopyOptions`. Canceling an edited choice
  leaves the previous default intact.
- `CPlane name` chooses Top, Bottom, Front, Back, Right, Left, or a saved
  construction plane.
- `View name` chooses an existing viewport's construction plane or the plane
  stored in a named view. Standard world names also work when no such view
  exists. Ambiguous viewport titles require a unique title or viewport click.

Options can also be supplied together:

```text
RemapCPlane CPlane=Front
RemapCPlane Copy=Yes CPlane "Mounting plane"
RemapCPlane View="Assembly view"
```

An identical source and target is rejected for both Copy settings; the
interactive prompt stays open for another target. After a missing or identical
named target, enter another name directly. Enter or Escape cancels a
pending placement. Accepted geometry and copied group memberships share one
Undo step. Copied group definitions remain empty after Undo and recover their
memberships on Redo, following the existing transform history policy.

For headless commands, `CommandContext.construction_plane` supplies the source.
World `CPlane` targets work directly. Arbitrary target frames can be supplied as
`TargetFrame=origin;x-axis;y-axis`, each vector written as three comma-separated
coordinates. Axes describe directions independently of document distance units.
Named CPlane and View resolution belongs to the application.

The built-in Bottom target uses the measured RemapCPlane basis `(-X,+Y,-Z)`.
It is distinct from the construction-plane editor's existing Bottom preset.

The [validation record](../remap-cplane-validation.json) retains 38 Rhino 8.32
commands and two canceled Copy-option workflows captured on private Xvfb. The
application replays each main command through complete, incremental and named
getter input, comparing full point-cloud, rational-curve, surface and mesh
definitions, grip display, source identity, names, groups, selection and
Undo/Redo at `1e-9`. Generated group names are compared by their naming pattern
and topology because Rhino's numbering spans the capture session.

Another 48 analytic cases independently check local coordinates and history
across model scales `2^-600` through `2^600`, with direction-vector scales chosen
independently of model scale. Parser/error tests preserve object state, selection and redo history;
application checks cover viewport routing, source-frame retention and
cancellation. These cases do not establish record-history dependencies,
dynamic preview appearance, arbitrary numerical ranges, embedded quotes in
saved names, or Rhino's near-coincident-plane admission threshold.

```sh
cargo test -p viboceros-command --lib remap_cplane
cargo test --bin viboceros app::tests::remap_cplane
tools/rhino_oracle/run_headless.sh rhino tools/rhino_oracle/fixtures/remap_cplane.json --scheme VibocerosOracleRemapCPlane --timeout 600
```
