# BlendCrv Alt editing

[Command guide](commands/blend-curve.md) · [Validation record](blend-curve-alt-validation.json)

Hold Alt when selecting a surface shape handle to change its angle in the
supporting tangent plane. The physical handle length stays fixed. Typed numbers
set degrees; points and viewing rays determine a direction in that plane.
Backward and negative directions are allowed. Shift does not couple angle edits,
including when the other handle has a different length.

The modifier is captured from the press event and lasts for that adjustment.
Releasing Alt before mouse-up or before picking the destination keeps the angle
edit. Pressing Alt only at the destination leaves a length edit. Ordinary curve
handles and contacts retain their existing constraints. Esc discards the local
candidate, and the accepted blend commits in one undo entry.

Control selection uses the nearest visible projected handle without a fixed
pixel-radius cutoff. A pre-angled input demonstrates why the actual rounded
input pixel matters: it chooses the contact and moves it to the boundary corner,
preserving the angle. A distinct prescribed pick selects the shape handle and
changes the angle. Distant surface and ordinary-curve picks check this behavior.

Four terminal-zero private Xvfb captures retain 38 observations, with 37 matched
document workflows. One exposes independent secondary shape-handle editing; it
is retained as a pending diagnostic, and selecting that control without Alt
leaves the preview unchanged. An earlier
eight-case pilot published complete observations, then its outer process exited
143. Those observations remain diagnostic replays; every pilot recipe repeats
in the clean verification capture. Frozen producers and all terminal receipts
are retained. The Native output geometry never seeds Rust construction.

Both builds replay 45 geometries, identities, attributes, selection, history,
1,452 independent point/tangent stations, 183 blend derivative stations and 90
3DM round trips. The nominal set has 36 accepts and one cancellation preserving
pending redo. Windowless egui tests also exercise press/release, held dragging,
late Alt, cancellation, numeric angles and atomic invalid directions.

Geometry and normalized derivative gates remain `1e-6`; parameter domains retain
`max(2e-6, 1e-8 * native span)`. Full Native drag/Escape trajectories, arbitrary
occlusion/depth ties, shared/seam/trimmed/closed boundaries, singular frames,
mathematical general G3/G4 continuity, continuous distance and comparative speed
remain unqualified. Independent secondary shape controls still require additional
jet freedoms; Alt rotation of their supporting face is available. The earlier
long-handle domain diagnostics remain separate.

```sh
python3 -m tools.rhino_oracle.check_blend_curve_alt
```
