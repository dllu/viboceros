# Transform scalar defaults

[Transforms](transforms.md) · [Copies and history](transform-copy.md)

At the first scale-factor or rotation-angle prompt, Enter accepts the last
accepted value. Scale, Scale1D, and Scale2D start with factor 1. Rotate and
Rotate3D start without an angle default; Enter then ends without an edit.
Shear has no remembered angle default and Enter ends its angle prompt.

Numeric input and completed reference picks update the corresponding command's
default. Full argument registry invocations and interactive picks share that
memory. Each command and registry has independent state; Undo, Redo, and
RememberCopyOptions do not reset scalar defaults. These preferences last for
the current application session.

Scale1D accepts its numeric factor before the direction pick. Canceling that
pick retains the factor, including zero or a negative value. Scale and Scale2D
reject zero and use the magnitude of a negative numeric factor. Scale1D retains
its sign. A rejected zero factor does not replace Scale or Scale2D's default.

With Copy=Yes, Enter at the first scalar prompt accepts the default and makes
a copy. Enter after an accepted edit finishes the batch. Canceling a repeated
session retains defaults from its accepted inputs and keeps accepted copies.

[ScaleNU](scale-nu.md) remembers independent X/Y/Z factors and accepts their
defaults separately. Its defaults update only after a complete three-axis set;
canceling a partial set retains the previous defaults. Its Rigid option is saved
immediately, including canceled commands, while WorldCoordinates resets to No.

[ScalePositions](scale-positions.md) remembers a positive factor and its 1D/2D/3D
mode on completion. Canceling numeric direction or mode input retains the previous
completed defaults. Numeric zero is rejected; a completed zero-length second
reference saves the mode without replacing the factor or editing geometry.

## Verification and limits

The [80 ordered recipes](../../tools/rhino_oracle/fixtures/transform_copy_default.json)
seed values through real commands before using Enter. The
[raw Rhino 8.32 observations](../../tools/rhino_oracle/observations/transform_copy_default.json)
retain every seed, cancellation, command history, terminal event, geometry,
attribute, selection, object order, SelLast, and external Undo/Redo snapshot.
No observed defaults are inserted into the application test's inputs or state.
Tests replay both interactive seeds and complete registry invocations followed
by interactive Enter, comparing point witnesses and document snapshots at
absolute epsilon `1e-9`, relative zero. Each case constructs fresh sources while
retaining command preferences.

Native reference-angle defaults can use a different winding, such as -270
degrees for a quarter turn. The application remembers its computed signed
reference angle. Its default produces the same measured rotation, but the
displayed winding does not match every native prompt. Raw native histories keep
that difference. Mouse dragging, persistence after restarting, and every
unsupported geometry or invalid-input combination remain unverified here.

```sh
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.transform_copy_capture tools/rhino_oracle/fixtures/transform_copy_default.json --scheme VibocerosOracleTransformDefaultFresh --timeout 300
cargo test -p viboceros transform_copy --bin viboceros
cargo test -p viboceros-command transform_default_tests --lib
python3 -m unittest tools.rhino_oracle.test_transform_copy
```

Use a fresh private settings scheme for this session fixture. Capture requires
the dedicated Xvfb wrapper. The standalone geometry replay CLI does not accept
these interactive operations.
