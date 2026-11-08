# Chamfer contacts and consumed open curves

[Command syntax](commands/chamfer.md) · [Validation record](chamfer-open-validation.json)

Distinct open sources use contact selection followed by setbacks on the picked
side. Interior line, arc and NURBS crossings keep the intended remainders.
Zero distances insert no bevel, but trimming still records source replacements
and clears pending Redo when the geometry is identical. Without trimming, zero
distances are a no-op only after a contact is found; parallel and skew pairs fail.

Setbacks are Euclidean sphere crossings. When a piece is completely consumed,
separate outputs retain the original piece while joining omits it. The remaining
bevel and other piece can form a valid join, including a bevel-only result when
both pieces are consumed. The first source keeps its identity and attributes.

Oversized line cuts can yield surprising valid separate outputs: a length-4
line with distance 9 retains an end at x=13 while its bevel starts at x=-5.
The retained cut parameter reflects about the original near endpoint; the bevel
uses the actual setback. Circular setbacks beyond the original arc can wrap into
a major arc. Tangent NURBS consumption locates the bevel on the far-end tangent
and extends the retained near end. Smooth NURBS consumption can fail atomically.

Collinear Line supports use 100 pair-bounds diagonals per end. Smooth replacement
supports use 50 original source spans; positive-distance bevels use 50 pair-bounds
diagonals. Thus retained pieces and bevels can be disconnected. Zero-distance
Smooth joins average the retained contact points while keeping their native
parameter intervals. Length, reversal, upper/lower pick, overlap and gap controls
verify these empirical rules. Shared contact code also serves zero-radius Fillet.

Five immutable private-Xvfb captures contain 69 outcomes: 62 successes, six
failures and one cancellation. Four successful joins create invalid disconnected
PolyCurves. Their public validity, domains and endpoints are retained; Viboceros
rejects them without changing geometry, attributes, groups or pending Redo.
They have preservation guards and no invalid-output geometry-parity claim.

Public UndoRedo begin/end pairs distinguish 61 records from 59 ordered scene
changes, including two equal-geometry replacements. A matching modern witness
corrects the inferred Undo expectation of one old zero-distance getter fixture.
The formerly excluded oversized fixture matches its fresh native repeat exactly.
The original raw archives remain intact.

Native displays round distances. Comparisons use accepted numeric tokens at full
precision; a distance of 3.9999999 on a length-4 line retains a valid 1e-7 piece
even though the prompt displays 4. The UI keeps that exact stored preference.

Both builds compare 58 explicit successful commands and 65 valid UI outcomes.
Ten API rejections and four invalid-output UI guards verify preservation. There
are 128 3DM round trips per build, including a windowless mouse test that checks
two source picks, fresh equal-replacement storage and one Undo. Coefficients,
domains, metadata, history and 9,603 public point/tangent stations use a 1e-6 gate.

```sh
python3 -m tools.rhino_oracle.check_chamfer_open
```

Application tests open no windows. Authored sources, physical sight points and
accepted input tokens supply inputs; native outputs are expectations only.
Coverage is finite: arbitrary folded or singular curves, every intersection
branch, tied costs, arbitrary parameterizations and tiny geometric limits remain
unqualified. There is no continuous-distance, pixel-equality or comparative
performance claim. Same-parent and closed-source adapters retain separate
qualifications.
