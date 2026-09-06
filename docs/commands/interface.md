# Display and drafting controls

[Command reference](README.md) · [Viewport navigation](../interface.md)

These application commands change interface state, not document geometry. They
remain available while a point command is unfinished and appear in completion
and `Help`. `Help UI` lists their syntax and shortcuts without cancelling a prompt.

| Command | Behavior |
| --- | --- |
| `Snap` | Toggle the one-unit grid snap. |
| `SetSnap On\|Off\|Toggle` | Set or toggle grid snapping. |
| `DisableOsnap Enable\|Disable\|Toggle` | Enable, suspend, or toggle object snaps. |
| `SmartTrack On\|Off\|Toggle` | Set or toggle reference-point axis tracking. |
| `SetDisplayMode [Viewport=Active\|All] Mode=Wireframe\|Shaded\|Ghosted` | Change the active viewport (default) or all four viewports. |

Names/options are case-insensitive, with optional Rhino-style underscore prefixes.
Commands accept a leading hyphen and optional transparent-command apostrophe.
`SetDisplayMode Shaded` and `SetDisplayMode Viewport All Mode Ghosted` also work.
Supply the complete options on one line; bare option-taking commands show usage
instead of starting another prompt. Unknown/duplicate options are rejected
before mutation, and invalid input remains editable.

For example, enter these lines individually:

```text
Line
0
SetSnap Off
DisableOsnap Disable
SetDisplayMode Viewport=All Mode=Ghosted
r4,3
```

The line still starts at the accepted origin. Interface changes do not consume
model undo steps or destroy redo history. `DisableOsnap` uses **Enable/Disable**,
not On/Off; the toolbar's Osnap indicator is lit when snapping is enabled.

F9 toggles grid snap; F4 toggles object snaps. Ctrl/Cmd+Alt+W, S, and G select
Wireframe, Shaded, and Ghosted in the active viewport. These shortcuts work while
editing coordinates, ignore key auto-repeat, and leave unrelated shortcuts and
text-editor undo alone. F3 and F11 are not drafting toggles. The view-preset menu
changes the active viewport's existing Top/Perspective/Front/Right preset; it is
not an implementation of Rhino's complete `SetView` or `CPlane` commands.

## Scope and validation

`viboceros-command::interface` owns the typed parser and state transitions.
The egui adapter and native oracle share this implementation. Interface commands
are intentionally separate from the document-only command registry. Automated
egui tests cover prompt/plane/selection preservation, partial typing, shortcuts,
toolbar clicks, narrow layouts, completion, and undo/redo isolation.

`tools/rhino_oracle/fixtures/interface_commands.json` compares eight initial
switch combinations, every active-viewport index, and 208 command transitions
with actual Rhino 8.32 macros; all 208 transitions match exactly. Both engines
record the initial state and every
subsequent state. These probes are untimed; they test settings, not visual
equivalence or rendering performance. The worker whitelists macros and restores
application settings and viewport modes in `finally`; failure-path unit tests
exercise initialization, command, and recording errors. A forcibly terminated
Rhino process cannot execute that cleanup.

Current limits: snap modes are the existing fixed Point/End/Mid/Center/Quad set;
grid spacing is one unit; SmartTrack is reference-axis tracking, not Rhino's
complete inference system. Per-mode Osnap selection, SnapSize, custom display
modes, UI-setting persistence, and full command macro interpretation remain
unimplemented. The supported controls follow McNeel's documentation for
[Snap/SetSnap](https://docs.mcneel.com/rhino/8mac/help/en-us/commands/snap.htm),
[object snaps](https://docs.mcneel.com/rhino/8/help/en-us/user_interface/object_snaps.htm),
[SmartTrack](https://docs.mcneel.com/rhino/8/help/en-us/commands/smarttrack.htm), and
[SetDisplayMode](https://docs.mcneel.com/rhino/8/help/en-us/options/view_displaymode_options.htm).
