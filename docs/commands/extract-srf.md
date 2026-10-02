# Extract surfaces

[Command reference](README.md)

`ExtractSrf` separates exact faces into independent surface objects. Start the
command, click faces or select them with a viewport rectangle, then press Enter.
Ctrl/Command removes faces from the picked set. Ctrl/Command+Shift face
preselection is also accepted. Picks can span multiple objects; geometry stays
unchanged while picking. Escape cancels, and `None` clears the pending picks.

Enter `Copy=Yes|No` or `OutputLayer=Input|Current` while selecting, including after
face preselection. Copy defaults to No and the output layer defaults to Input.
With Copy=Yes, sources remain unchanged. Otherwise an unextracted remainder
retains the source identity, attributes, and group memberships; a fully extracted
source is deleted. Extracted faces inherit source attributes, become selected,
and have no group memberships, as described in
[McNeel's command help](https://docs.mcneel.com/rhino/8/help/en-us/commands/extractsrf.htm).
Current output changes only their layer.

Sources are processed in reverse document order, with faces in descending index
order. Each source's extracted faces precede its renewed remainder. This order
matches the saved native runs, including reordered and repeated selections.
One Undo restores the entire extraction. Changed or unavailable face sources
are rejected before editing.

## Typed selectors and API

Explicit selectors execute immediately on whole-object selection:

```text
ExtractSrf Faces=0,2 Copy=Yes OutputLayer=Current
ExtractSrf Faces=All
ExtractSrf Face=0 Object=<selected-object-id> Copy=No
ExtractSrf 2,1,0
```

`Faces` applies to every selected source. A point selects the nearest exact face
among the selected sources. Lists use zero-based indices. The typed selectors
also accept standalone untrimmed NURBS surfaces as one face.

The command crate exposes independent staging for different face sets on
different objects:

```rust
let extraction = viboceros_command::ExtractSurfaceSelection::prepare(
    &document, [(first_object, 0), (second_object, 2)], false, true,
)?;
extraction.commit(&mut document)?;
```

All targets are validated before staging, and duplicate targets are idempotent.
Commit checks source geometry and order, attributes, model tolerance, and the chosen
current layer. No fitting or tessellation is used to construct extracted faces
or remainders; existing NURBS definitions and trim curves are retained.

## Verification and limits

The [60-case fixture](../../tools/rhino_oracle/fixtures/extract_srf_faces.json) and
[raw Rhino 8.32 captures](../../tools/rhino_oracle/observations/extract_srf_faces.json)
cover joined and disconnected faces, individual and complete extraction, mixed
objects, duplicate targets, reordered and repeated inputs, both Copy modes, both
output layers, six-face boxes, cylinder seams, signed rational trim curves, and reversed faces.
Every case includes Undo/Redo. Comparisons retain raw object order and topology
indices, checking complete geometry definitions, identity, attributes, groups,
selection, and component clearing at absolute epsilon `1e-9`, relative zero.

```sh
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.extract_srf_capture tools/rhino_oracle/fixtures/extract_srf_faces.json --timeout 300
python3 -m tools.rhino_oracle.extract_srf_replay tools/rhino_oracle/fixtures/extract_srf_faces.json tools/rhino_oracle/observations/extract_srf_faces.json
cargo test -p viboceros-command extract --lib
cargo test -p viboceros-oracle extract_surface --lib
cargo test -p viboceros extract --bin viboceros
```

Native captures use public SDK face preselection followed by the actual command.
Application tests cover command-first clicks and rectangles, removal, options,
cancellation, stale picks, and history. Native physical mouse sequences are not
yet recorded here. The matrix does not establish native SubD editing, shared
`RememberCopyOptions` memory, or every imported trim representation.
