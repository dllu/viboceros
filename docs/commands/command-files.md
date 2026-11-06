# Command files

Enter `ReadCommandFile "path/to/model.txt"`, or enter `ReadCommandFile` and supply
the filename at the next prompt. Files feed the application's existing command
getters. For example:

```text
; Create a line, an open polyline, and two points
! _Line w0,0,0 w4,3,2
_Polyline w0,0,0 w4,0,0 w4,3,1 _Enter
_Point w1,2,3
_Point w4,5,6
```

Whitespace separates input; line breaks do not finish commands. Quote names and
filenames containing spaces. Semicolons start comments outside quotes. UTF-8,
UTF-8 with a BOM, and BOM-marked UTF-16 are accepted.

`_Enter` accepts the current prompt and does nothing at idle. `_EnterEnd` accepts
successive prompts until the current command finishes. A leading `!` cancels a
previous getter; a later `!` stops the remaining file. `_Pause` waits for a point;
`_MultiPause` waits until the user finishes the current getter. Escape cancels
the unfinished command and remaining file. `_NoEcho` and `_Echo` control scripted
command-log output. Nested `ReadCommandFile` resumes its caller on completion.

Completed edits in a file, including nested files, share **one Undo**. Canceling
or failing later input retains completed edits in that history entry. Ordinary
Undo/Redo is unavailable while reading a file, including its filename prompt;
supported getter-local Undo still works. `SelLast` refers to the last completed
child command. A canceled file without edits preserves pending Redo.

Staged `_-Layer` supports New, Current, On, Off, Lock, Unlock, Delete and List;
finish with `_Enter`. `New` creates a layer without changing the current layer.
The existing whole-line kernel extension `Layer New name` also sets it current.

Command-created polylines retain physical chord-length parameters, as observed
in native command output. The low-level `Polyline3::try_new` API retains the
OpenNURBS SDK convention of vertex-index parameters.

Files are bounded to 16 MiB, one million consumed inputs and sixteen nesting
frames. Execution yields between inputs; a long modeling operation still runs
synchronously. The standalone kernel registry does not run command files.
Commands lacking staged application getters and advanced repetition,
transparent-command and redraw macro syntax remain unsupported. Pause
qualification covers point getters, not arbitrary option prompts.

The [qualification record](../command-file-validation.json) compares 28 prescribed
native files in portable and native builds, with 26 3DM round trips per build.
It retains one layer Redo discrepancy: Rhino redoes the tested layer sequence
with the new layer locked and the previous layer current; local Redo restores
the original final state. Construction and Undo layer states match.
Coefficient and finite-sample comparisons are bounded by 1e-6; continuous
distance, complete trim ordering and comparative speed are unqualified.

Run the windowless checks with
`python3 -m tools.rhino_oracle.check_command_file`. Native captures use private
Xvfb sessions and frozen prescribed inputs; they do not open desktop windows.
