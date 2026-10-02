# Extract surfaces

[Command reference](README.md)

`ExtractSrf` separates exact faces into independent surface objects. Start the
command, click faces or select them with a viewport rectangle, then press Enter.
Ctrl/Command removes faces from the picked set. Ctrl/Command+Shift face
preselection is also accepted. Picks can span multiple objects; geometry stays
unchanged while picking. Escape cancels; `None` cancels and clears selection.

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

These 60 captures use public SDK face preselection followed by the actual
command. The [32 physical input sequences](../../tools/rhino_oracle/fixtures/extract_srf_picking.json)
and [raw native observations](../../tools/rhino_oracle/observations/extract_srf_picking.json)
also exercise Rhino's actual command-first selection prompt. They cover joined
and disconnected planar faces, multiple sources, repeated picks, Ctrl removal,
Ctrl+Shift toggling, window and crossing rectangles, rectangle modifiers,
Copy/layer edits after picking, Escape, and `None` before and after picking.
Rhino's `None` cancels extraction and invokes `SelNone`; our UI follows that
behavior. Undo/Redo is checked for every completed extraction. Cancelled commands
leave geometry unchanged and create no extraction history.

The physical cases replay through a headless application test. Independent input
recipes construct the source document. Press, move, and release events go through
egui and the viewport component picker, then through the application's command
handling. Native selection records are expected outputs only; no expected picks
are injected into the application. Geometry and metadata must stay equal to the
captured input document throughout picking. After execution and Undo/Redo they
are compared to the corresponding native states, with the same `1e-9` absolute
tolerance and raw topology/order checks as the geometry matrix. The oracle crate
exposes `ExtractFixture::prepare_document` and
`observe_component_document` for this application-level instrumentation.

```sh
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.extract_srf_capture tools/rhino_oracle/fixtures/extract_srf_picking.json --timeout 300
cargo test -p viboceros extract_surface_viewport_sequences --bin viboceros
```

All live captures run in a private Xvfb display and accept input only in the
newly owned Rhino window. Face-click locations are checked through public shaded
pick-frustum and face-intersection APIs before clicking. Mouse/key handlers and
pressed modifiers are released on failures; incomplete sequences are rejected.

The physical matrix covers planar sources in a shaded Top view. Other views,
curved-face mouse picking, native SubD editing, shared `RememberCopyOptions`
memory, and every imported trim representation remain unverified by these cases.
