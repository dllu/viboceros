# UV command face references

[ApplyCrv](commands/apply-curves.md) · [CreateUVCrv](commands/create-uv-curves.md)

Both commands accept `Surface=object-uuid Face=index` to reference one face of a
multi-face B-rep. A NURBS surface or one-face B-rep may omit `Face`; a NURBS
surface accepts only face zero. A multi-face object requires an index and never
silently chooses the first face. Duplicate options, invalid indices, missing
objects and restricted references fail before document edits. Argument order
is independent.

The shared resolver borrows the stored face and underlying surface. It does not
extract, replace or reorder the original B-rep. Apply maps World-XY input bounds
into the face's original natural domains; Create emits that face's rectangle
and trim loops. Properties and copied groups still come from the original object.

Reference prompts now enable selectable B-rep face hits. Apply target clicks
complete the command. Create clicks retain the face until optional extra inputs
are accepted. Typing `object-uuid Face=index` uses the same validation. A whole
multi-face preselection asks for a face; Create can also use one valid preselected
face component. Escape restores the original whole-object selection. Invalid
references keep the prompt available for retry.

## Native evidence

The [14 closed recipes](../tools/rhino_oracle/fixtures/uv_face_reference_command.json)
and [raw records](../tools/rhino_oracle/observations/uv_face_reference_command.json)
ran on private Xvfb. Six Create and six Apply recipes successfully used public
component-face preselection. Two coordinate-only Apply attempts were rejected
and remain recorded as failures. Successful captures retain source surfaces,
output curves and 33 stations per output, command-end events and Undo/Redo.

Native `BoundingBox.ToBrep` and the kernel box constructor have different face
and chart conventions. Native replay reconstructs the captured source charts
inside an independent six-face B-rep, without changing expected output. This
replay does not reconstruct native shared-edge topology. Separate tests exercise
every face of the kernel's connected box, original face-domain mapping, source
purity, history and rejected references. Application tests cover viewport hits,
typed indices, retry and cancellation. Existing sizing/projection discrepancies
remain documented in the command pages.

See [provenance](uv-face-references-provenance.json). Passing these cases does
not establish arbitrary-geometry, constructor-indexing or backend parity.

```text
ApplyCrv Surface=object-uuid Face=5
CreateUVCrv Face=2 Surface=object-uuid
```
