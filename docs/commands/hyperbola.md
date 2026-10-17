# Hyperbola

Start `Hyperbola` to pick a center, focus and end. Choose a construction at the
first prompt, or start it directly:

| Construction | First point | Second point | Third point |
| --- | --- | --- | --- |
| Default / Center | Center | Focus | End |
| FromCoefficient | Center | Opening direction | End |
| FromFoci | First focus | Second focus | End |
| FromVertex | Vertex | Focus | End |

The getters use the shared typed/world/relative coordinate, unit, point-filter,
snap and drafting input. Curve, focus and asymptote previews do not edit the
model. Invalid points leave the getter available for a corrected pick. Escape,
Cancel or Enter at a point prompt abandon the construction. Starting another
command also discards its preview.

At the end prompt, `BothBranches`, `MarkFoci` and `ShowAsymptotes` accept `Yes` or
`No`, or toggle when entered alone. `MarkFoci` adds one focus for a single branch
and both foci for two branches. Focus objects precede curve objects in history.
Asymptotes are temporary construction guides.

In `FromCoefficient`, `A` and `B` are positive lengths. Enter `A=3mm` directly,
or enter `A` and then its value. Enter at the numeric prompt retains the current
coefficient and returns to the point getter. Drawing options and coefficients
are remembered after successful completion, including across undo and documents
sharing a command registry. A new command starts in center mode; selecting a
construction does not change that default. Cancelled and failed local drafts do
not save their settings.

```text
Hyperbola 0,0,0 5,0,0 3.75,3,0 BothBranches=Yes MarkFoci=Yes
Hyperbola FromFoci -5,0,0 5,0,0 3.75,3,0 BothBranches=No MarkFoci=No
Hyperbola FromVertex 3,0,0 5,0,0 3.75,3,0 ShowAsymptotes=Yes
Hyperbola FromCoefficient 0,0,0 1,0,0 5,2,0 A=3 B=4
```

Scripted options may follow the coordinates. Separate numeric `A`/`B` prompts
and units are interactive inputs; complete scripts use `A=number B=number` in
model units. The opening axis must not be parallel to the captured construction
plane's normal. The plane is captured at the first point and retained if the
active viewport changes.

## Observed Rhino getter behavior

The implementation includes several behaviors measured in Rhino 8.32:

- Choosing FromCoefficient, FromFoci or FromVertex at the center construction's
  **focus** prompt ends that command with Cancel. Choose the construction at its
  first prompt. Three live-input captures confirm the cancellation without
  timeout or an injected Cancel token.
- The coefficient end getter requires the picked point's **world X** coordinate
  to exceed A, independently of the chosen center or rotated opening axis.
  The extent used for the curve is then projected onto the opening axis.
  Translated, rotated and boundary cases verify this separate guard.
- If the accepted rotated extent lies between the vertices, Rhino can finish
  with focus markers only, or no objects when MarkFoci is off. Viboceros matches
  these results. Completions with objects form one undo edit; empty completions
  leave no model history entry.
- Points typed outside the captured CPlane can change the rational weights and
  produce projective conics. Viboceros preserves those weights and signed
  extents. Some charts have denominator poles even though Rhino reports the
  NURBS structurally valid. Length queries fail there; the oracle records
  Rhino's zero result with `length_query_failed=true`. This is not a zero arc
  length. The geometry API returns an error, and drawing/picking skips segments
  across the quadratic poles.

## Verification and limits

[The validation record](../hyperbola-validation.json) records 86 distinct native
Hyperbola cases: 63 successful commands and 23 cancellations/rejections. They
cover all construction modes, option combinations and toggles, numeric/default
coefficient prompts, remembered settings, rotated and spatial axes, CPlanes,
signed extents, focus-only/empty results, projective weights and actual undo/redo.
Exact controls, weights, knots, domains, object order, selection and finite
lengths are compared with absolute epsilon `1e-6` in both backends. Pole-related
length failures are checked separately. Two fresh Conic captures verify the
shared managed input driver with real hover and edge selection.

The analytic pole handling covers single clamped quadratic curves with equal
endpoint weights, including all Hyperbola outputs here. It is not a general
pole certificate for arbitrary NURBS. Near-singular CPlane angles, exhaustive
snap combinations, all projective downstream modeling operations, complete GUI
visual parity and performance parity remain unqualified.

The public construction modes are described in
[McNeel's command reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/parabola.htm#Hyperbola).
