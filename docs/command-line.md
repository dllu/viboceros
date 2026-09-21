# Command line editing

[Project overview](../README.md) · [Viewport controls](interface.md)

Start typing while a viewport is active to focus the command line. At the idle
command prompt, Tab selects the best completion and repeated Tab cycles through
matches; Shift+Tab cycles backwards. Click a suggestion to use it. Prefix matches
come first, followed by substrings, ordered-letter matches (for example `mshsph`
for `MeshSphere`), and small spelling corrections. Completion inserts text and
never executes a command.

Up recalls the last submitted command; further Up/Down presses browse history.
Down past the newest entry restores the text you were writing. Editing a recalled
command creates a new draft. Up also works from a viewport. These shortcuts do
not take over other text fields or active geometry/option prompts. Coordinate
responses inside a drawing command are not saved as separate commands.

History is saved when a command is submitted, including unsuccessful commands
that may need correction. Consecutive duplicate entries are collapsed. The last
1,000 entries from the journal's final 1 MiB are loaded at startup. History is
stored separately from geometry undo/redo and the on-screen output log:

- Linux: `$XDG_STATE_HOME/viboceros/command-history.jsonl`, or
  `~/.local/state/viboceros/command-history.jsonl`.
- macOS: `~/Library/Application Support/viboceros/command-history.jsonl`.
- Windows: `%LOCALAPPDATA%/viboceros/command-history.jsonl`.

The journal is append-only, with locked writes so separate application sessions
preserve each other's entries. Malformed/truncated records are skipped. New Unix
journal files use owner-only permissions. If saving fails, the command log reports
it and recall continues in memory. Delete the journal while the application is
closed to clear saved history.

## File paths

`Import3dm`, `Export3dm`, `ImportStl`, `ExportStl`, `ImportStep`/`ImportStp`, and
`ExportStep`/`ExportStp` suggest existing directories and files after the command
name and a space. `ExportStl Ascii|Binary` and `ImportStep Native=Yes` keep their
options ahead of the path. Matching is case-insensitive, but inserted filenames
retain their actual case. Hidden names appear when the filename prefix starts
with a dot.

Relative paths, absolute paths, and `~/` are supported; home-directory completion
inserts an absolute path. Completions quote paths, including spaces and Unicode.
Completing a directory adds its separator and places the caret inside the closing
quote: type a new export filename, or press Tab again to complete an existing
child. Suggestions can also be clicked to choose a different directory. Typing
resets the completion cycle. File completion does not read file contents or
create/overwrite files; Enter runs the chosen import/export command as usual.

## Regression checks

```sh
cargo test --release -p viboceros
cargo clippy --release -p viboceros --all-targets -- -D warnings
cargo fmt --all -- --check
```

Tests exercise actual egui frames for Tab/Shift+Tab, arrow recall, draft restoration,
focus isolation, and quoted directory editing. Separate temporary-file tests
check journal reloads and interleaved sessions, fuzzy ranking, and path grammar.
