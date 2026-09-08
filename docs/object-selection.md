# Object selection and confirmation

[MeshToNURB](commands/mesh-to-nurb.md), [ToNURBS](commands/to-nurbs.md),
[ConvertToBeziers](commands/beziers.md), and
[ConvertToSingleSpans](commands/single-spans.md) use these workflows. Other object-taking
commands do not yet share this interactive framework.

With no eligible preselection, enter the command and click objects or drag
selection windows. Ordinary clicks add picks without Shift; Ctrl/Command removes
them. Empty clicks leave picks alone. `SelAll` adds only eligible, selectable
objects; `SelNone` clears the picks. Hidden/locked objects and objects on hidden/
locked layers cannot be picked. A group's other members are not implicitly picked.

| Behavior | MeshToNURB | ToNURBS | ConvertToBeziers |
| --- | --- | --- | --- |
| Eligible picks | Meshes | Curves, surfaces, B-reps, meshes | Curves, surfaces, single-face B-reps |
| Enter after picking | Converts | Opens confirmation; no-op-only input finishes | Opens deletion question |
| With preselection | Converts immediately | Opens confirmation unless all inputs are no-ops | Asks deletion; explicit answer executes immediately |
| Option edits | Accepted immediately | Staged until a real conversion succeeds | Yes/No answer converts immediately |
| Cancelled choices | Remembered | Discarded | Discarded |

At the MeshToNURB prompt, type `TrimTriangularFaces=No`, `UseNgons=No`, or both.
Paired forms such as `TrimTriangularFaces No` are also accepted.

For ToNURBS, finish picking first. At confirmation, set `DeleteInputObjects=Yes|No`.
If meshes were picked, `MeshOptions` opens a triangle-trimming submenu. Enter
returns from the submenu to the main options; another Enter converts. Flat or
inline mesh options and the `DeleteInput` alias are also accepted. Invalid or
duplicate options leave the whole input editable and do not partially accept it.
Mesh-only options are unavailable for curve-only selections.

For ConvertToBeziers, finish picking first, then answer `Yes` or `No` to delete
or keep the inputs. Enter uses the displayed choice. Named deletion options also
answer the question; no additional Enter is required. Invalid/duplicate input
keeps the question open without changing geometry or remembered choices.

ConvertToSingleSpans picks only surfaces and single-face B-reps. Enter opens
options, including for no-op-only inputs; preselection opens them directly.
`Direction=U|V|Both` and `DeleteInput=Yes|No` update options, and Enter converts.
Bare `Direction` opens a value chooser; selecting a value or pressing Enter returns
to options. `Toggle` switches U/V and is unavailable in Both. These edits are
remembered immediately, even if conversion is cancelled; cancellation during
selection accepts nothing. Its Direction chooser takes precedence over the
global `Direction` alias for `Dir`.

Escape cancels the entire pending command, including from a submenu. Command-first
picks and initial ineligible selection are cleared; preselection is retained.
Cancellation makes no geometry edit, adds no undo entry, and does not discard redo.
Starting another modeling command cancels the pending one first.

Display, grid snap, Osnap, SmartTrack, and CPlane controls preserve pending state.
A nested CPlane prompt temporarily uses point input; cancelling it restores the
previous selection/confirmation/submenu phase. Confirmation freezes object picks,
but still permits viewport navigation. Undo/Redo toolbar buttons and the
Delete-object shortcut are disabled while a prompt is unfinished. Successful
conversion is one undoable edit.

## Separation of responsibilities

`viboceros-command::object_selection` defines geometry filters, typed boolean
options/aliases, menus, finite choices and two-value actions, and workflow descriptions. The command supplies a
selection-dependent confirmation description; no-op-only input can bypass it.
Reading descriptions and validating edits do not change document state.
Command-owned acceptance hooks distinguish immediate option memory from deferred
confirmation. Selection-only phases do not call options-stage acceptance hooks;
immediate selection-stage options are a separate workflow. `execute_postselected` shares normal transaction/rollback handling
while allowing different selection cleanup and creation ordering.

`app/object_selection` owns phase and pre/postselection origin, separately from
point drafting and nested construction-plane input. The viewport receives an
optional object filter alongside independent drafting input: no filter disables
picking without turning on point drafting. Filtering happens before hit priority
and window/crossing queries. The app filters again before selection mutation and
rejects click/window changes during confirmation.

Tests cover real egui pointer events, disabled selection, aliases, submenu scoping,
atomic invalid input, additive/removal picks, groups, hidden/locked objects,
cancellation, undo/redo, nested CPlane input, and transparent controls. Rhino
comparisons and private-Xvfb native GUI exports are detailed in each command reference.
