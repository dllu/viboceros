# Remember Copy options

[Command reference](README.md)

```text
RememberCopyOptions Yes
RememberCopyOptions No
```

The application starts with remembering enabled. Enter the bare command in the
GUI to answer Yes or No; Enter accepts the displayed setting, and Escape or
`Cancel` leaves it unchanged. A bare registry/Python invocation reports the
current setting without opening a prompt.

Copy choices are separate for each command. A completed edit saves its choice,
including when remembering is disabled. With remembering enabled, omitting
`Copy=` uses that command's saved choice. With remembering disabled, starting a
command resets its saved choice to its built-in default. That reset survives
cancellation; transient option edits do not. Commands that have not started
retain their saved choices for use when remembering is enabled again.

For example, completing `Rotate ... Copy=Yes` while remembering is disabled and
then immediately enabling remembering gives the next Rotate a Yes default.
Starting another Rotate while remembering is disabled resets that value to No,
even if the second command is cancelled.

The shared policy currently applies to these implemented commands:

| Commands | Initial Copy default |
| --- | --- |
| Mirror, OrientOnSrf | Yes |
| ExtractSrf, ExtractSubCrv, RemoveFromGroup | No |
| Orient, Orient3Pt | No |
| Scale, Scale1D, Scale2D, ScaleNU, ScalePositions | No |
| Rotate, Rotate3D, Shear, SetPt | No |

Aliases use the same saved choice. A registry retains preferences across its
documents; separate registries are independent. Settings and queries create no
model history and preserve redo. Geometry Undo/Redo does not revert preferences.
Invalid options, ineligible selections, and failed geometry edits do not accept
an edited Copy value. With remembering disabled, the command-start reset still
applies to a failed command.

The GUI's ExtractSrf, SetPt, RemoveFromGroup, Scale, Scale1D, Scale2D, ScaleNU,
ScalePositions, Rotate, Rotate3D, Mirror, and Shear prompts use the shared defaults
and save completed choices. The affine prompts support [Copy editing and repeated targets](transform-copy.md).
Each accepted target saves its choice; a later transient option edit followed
by Enter or Escape does not replace it. Other implemented transform prompts
apply saved choices when executing their completed point input; their option
editing and repeated target workflows remain incomplete. Use explicit
`Copy=` in full command invocations for deterministic scripts. Preferences
currently last for the application session and are not saved across restarts.

## Native verification

The [independent 50-step fixture](../../tools/rhino_oracle/fixtures/copy_options.json)
and [raw Rhino 8 observations](../../tools/rhino_oracle/observations/copy_options.json)
interleave ExtractSrf, Rotate, Scale, Mirror, and the global setting in one
private settings scheme. Each step creates fresh owned geometry. They cover
omitted choices, successful edits, transient edits cancelled before execution,
disabled command-start resets, re-enabling, and independence between commands.
The ExtractSrf input is exported by our independent B-rep builder before capture.
Native public SDK face preselection precedes the actual scripted command;
cancellation runs without preselection so it occurs before extraction.

The replay compares complete before/after geometry, raw object order, source
identity, and selection at absolute epsilon `1e-9`, relative zero. B-rep
control points, knots, domains, tolerances, and topology indices are retained.
Transform defaults are read from a cancelled, unedited native prompt. This query
never changes the remembering setting or supplies a Copy answer.

Raw command histories and EndCommand events are stored unchanged. Rhino reports
`Nothing` when Enter ends Rotate's Copy=Yes repetition after creating a copy;
Scale reports `Success`. Our single-target registry returns success for the
completed edit. Replay verifies the edit separately from that native termination
status. This capture does not verify repeated-copy history grouping, native
Undo/Redo, restart persistence, or option lifetime for the remaining commands
in the table. A separate [49-case interactive transform capture](transform-copy.md#verification-and-limits)
verifies repetition, SelLast, and native Undo/Redo for seven commands.
Core tests cover history independence and aliases, and application
tests cover prompt completion and cancellation.
The separate [ScalePositions capture](scale-positions.md#verification-and-limits)
verifies its copies, original group memberships, defaults and Undo/Redo.

```sh
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.copy_options_capture tools/rhino_oracle/fixtures/copy_options.json --scheme VibocerosOracleCopyFresh --timeout 300
cargo test -p viboceros-oracle copy_defaults --lib
cargo test -p viboceros-command copy_options --lib
cargo test -p viboceros copy_options --bin viboceros
python3 -m unittest tools.rhino_oracle.test_copy_options
```

Use a fresh scheme name beginning with `VibocerosOracle` for reproduction. Capture
requires a private scheme and dedicated Xvfb, validates bounded workflows before
launch, exports only to temporary owned paths, and rejects incomplete responses.
It does not change the shared desktop Rhino profile.

McNeel documents the affected command set under
[General options](https://docs.mcneel.com/rhino/8/help/en-us/options/general.htm)
and the scheme startup argument under
[starting Rhino](https://docs.mcneel.com/rhino/8/help/en-us/information/startingrhino.htm).
