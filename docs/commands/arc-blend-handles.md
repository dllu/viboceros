# ArcBlend adjustment handles

[ArcBlend](arc-blend.md) · [Curve commands](curves-index.md)

After selecting two source curves, three handles appear: the two contacts
and a ratio handle on the straight chord between them. Click a handle, then
move and click to accept its preview, or press, drag and release it.
The model changes only when Enter finishes the whole command.

Endpoint handles slide along the original bounded source curve. A typed
point projects to that curve, including clamping at its endpoints. The
selected source end still determines the tangent orientation and which
portion Trim keeps.

The ratio handle lies at the fraction `r/(1+r)` along the endpoint chord.
It stays on that chord when AlternateSolution changes the arc branch.
With this handle selected, type a positive ratio or a point. Typed points
and object snaps project in 3D; free mouse movement uses the closest point
between the chord and viewing line. This differs from screen-space curve
location in Perspective. The implementation reuses the Move/Copy constraint.
`RadiusDifference` also works inside this getter; its numeric prompt accepts
the displayed default on Enter and returns to ratio editing.

Esc discards the current adjustment preview and returns to handle selection.
Another Esc cancels ArcBlend. `!` cancels the whole command. Deleted or
changed references clear the editor and require new source picks. Cached
previews and immutable source references avoid document edits and source
curve copies on each mouse move.

[Validation](../arc-blend-handles-validation.json) retains 44 new Rhino
observations, including 26 matched observations from 24 distinct input
recipes and 18 diagnostics. Both builds check NURBS coefficients, domains,
attributes, selection, history and 3DM round trips. Windowless egui tests
exercise clicks, held drags, previews, Escape and stale references. Reproduce:

```sh
python3 -m tools.rhino_oracle.check_arc_blend_handles
```

The native ratio control is separate from the arc junction. Early probes
aiming at the junction missed it in some views; fresh chord picks enter the
getter in Top, Perspective and the alternate branch. The captures retain
physical mouse acknowledgements, public viewing rays and selected public
projection matrices. Output coordinates do not seed the Rust construction.

Zero ratio and end snaps restore the last radius target within the command,
initially zero. The [general radius search](arc-blend-radius.md) also reproduces
failed-target previews for nonparallel tangents and alternate solutions. Native endpoint comparisons cover line
sources. Nonlinear contacts, edge adjustments, exact hit-box/depth ranking,
SubCrv and remembered defaults remain unqualified or unsupported.

Rhino keeps source IDs while assigning new runtime serials to trimmed
objects. The controlled comparison matches its unique source names; object
enumeration order remains unqualified. Geometry is compared through exact
NURBS data and 33 local evaluations. The newer parallel-radius qualification
adds independent native command arc samples. Continuous distance and
comparative speed remain unqualified.

Reference: [Rhino 8 ArcBlend](https://docs.mcneel.com/rhino/8/help/en-us/commands/arcblend.htm).
