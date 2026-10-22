# Application input and UI

[Architecture](../architecture.md) · [Development and testing](../development.md)

`src/app.rs` owns application state, construction, the command-line entry point,
and the transactional command adapter. Command-specific sessions and their
previews stay in individual `src/app/` modules.

| Module | Responsibility |
| --- | --- |
| `interactive_state/` | Interactive command variants, names, prompts and drafting anchors |
| `interactive_start` | Parse initial arguments and start command-specific sessions |
| `drafting_input` | Dispatch accepted points and snaps to the active command |
| `interactive_cancel` | Complete or discard interactive sessions and release their picks |
| `prompt_cancel` | Route Escape through transient prompts and selections |
| `selection_input` | Selection clicks, ambiguity menus, fences and window regions |
| `component_selection` | Transient component picks and source snapshot validation |
| `viewport_input` | Route viewport output to selection, drafting and active tools |
| `ui` | Frame layout, keyboard shortcuts, panels and viewport rendering |
| `sidebar` | Apply document sidebar actions through transactions |

These modules extend `VibocerosApp`; their shared methods are visible only inside
the application module. Moving a handler does not change its document, selection,
or history contract. Geometry algorithms and command admission remain in their
workspace crates.

Application tests live in `src/app/tests/`. The root contains shared fixtures and
frame helpers, while topic modules cover command input, primitives, editing,
selection, transforms and command-specific sessions. Add regressions beside the
relevant topic instead of growing the application root.

The [refactor checkpoint](../application-refactor-validation.json) records source
preservation and regression checks.
