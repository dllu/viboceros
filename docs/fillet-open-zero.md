# Zero-radius Fillet between open curves

[Command reference](commands/fillet.md) · [Validation record](fillet-open-zero-validation.json)

Radius zero inserts no arc. With two distinct open sources, `Trim=Yes` trims or
extends each source to the chosen contact, keeping the picked side at interior
crossings. Source IDs, attributes and groups survive. Join combines the retained
pieces with the first picked source's identity. `Trim=No` succeeds without an
edit, even for disjoint or skew sources. Trimmed skew or out-of-support pairs fail
without altering the document or pending Redo.

An equal-geometry replacement is still a Fillet Undo record and clears Redo.
Dynamic preview leaves the sources and history intact; acceptance commits its
cached candidate once. Escape preserves pending Redo. Arc and NURBS
continuations honor the extension options. Actual spatial intersections work
without requiring coplanar lines.

Headless commands without explicit picks use the nearest source endpoints. If a
default pick coincides with a source-start contact, the available nonempty side
is retained. Library regressions cover both joined and separate replacements;
the native workflows use physical picks near their intended contacts.

Native collinear behavior can produce very long extensions. With
`ExtendOtherCurvesBy=Line`, each support is padded by 100 times the pair's original
bounding-box diagonal at each end. With `Smooth`, line supports are padded by 50
original source spans. Summed travel from the physical picks ranks the two
overlap ends, so pixel offsets near a tie affect the result. Smooth retains each
source's own support endpoint; Join averages those endpoints while retaining
their parameter intervals. These empirical rules are checked across lengths,
reversals, translations and orientations, including a bounded failure control.
The native intersection bridge bounds endpoint roundoff to 64 floating-point
epsilons of its parameter scale, then independently checks original-curve
residuals and overlap interiors. A producer regression covers 64 padded line
overlaps across orientations, translations, lengths and direction reversals.

Five private-Xvfb captures contain 49 valid native outcomes: 45 successes, three
failures and one cancellation. Public UndoRedo begin/end pairs distinguish 42
records from 39 ordered scene changes. Three replacements have identical scene
values. Two old getter fixtures had inferred history from geometry; their raw
archives stay intact, with derived history expectations tied to a matching
modern event-observed case.

An earlier eleven-case overlap capture stalled at a private Selection Menu
because the long-line picks projected one pixel apart. Its request, frozen
producers, failure log and menu inspection are retained with zero observations.
Only its verified owned Rhino process was terminated. The complete request was
repeated with separated long-line picks; all eleven outcomes are included.

Both builds replay 45 explicit commands and all 49 UI outcomes, with 94 fixture 3DM round
trips per build. A windowless mouse test adds one archive round trip and checks
two source picks, cached preview, equal replacement storage and one Undo. Coefficients, domains, attributes,
history and 6,204 public point/tangent stations are checked at a 1e-6 model gate.

```sh
python3 -m tools.rhino_oracle.check_fillet_open_zero
```

Application tests open no windows. Authored definitions and pre-command physical
sight points are inputs; native outputs are expectations only. Coverage is finite:
arbitrary overlaps, tied pick costs, other parameterizations, singular or folded
continuations, arbitrary nonplanar curves and tiny positive radii remain
unqualified. There is no continuous-distance, pixel-equality or comparative
performance claim. [Closed-source zero behavior](fillet-zero.md) and
[same-parent behavior](fillet-self.md) have separate coverage.
