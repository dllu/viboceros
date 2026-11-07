# Fillet on one parent

[Command reference](commands/fillet.md) · [Validation record](fillet-self-validation.json)

Pick two locations on one polyline, polycurve or kinked NURBS to fillet a junction.
Trim keeps the two ends of an open parent or the cyclic remainder of a closed
parent. Join replaces the parent in place, preserving its ID, attributes and
groups. Without Join, the first remainder retains that identity; a second open
remainder is new, ungrouped geometry with the parent's attributes. A separate arc
uses fresh attributes on the current layer. Trim=No keeps the complete parent.

Closed remainders and joined results follow Rhino's seam orientation and native
parameter domains. Reversing the pick order does not reverse the parent's result.
Native-domain seed distances rank neighboring junctions for nonadjacent picks.
Coincident active line-offset branches reject a singular angle. A closed NURBS
seam can form a junction even without an interior kink. Existing portable/usd-brep
producers solve contacts and extract retained intervals. Batch C0 splitting copies
only the output control blocks and preserves exact rational weight gauges.

The same prepared edit plan supplies dynamic previews and atomic commits. Two
mouse picks on one object produce one selected parent. Numbers and options update
the preview; Enter accepts and Escape cancels. Successful no-ops and failures
preserve pending Redo. Zero radius on one parent fails without an edit. Picks in
one smooth piece can finish successfully without constructing a fillet.

Headless calls accept repeated explicit source IDs or one selected parent with
both Pick1 and Pick2 supplied. See the command reference for syntax.

Five immutable licensed-Rhino captures contain 38 complete workflows: 28 edits,
two successful no-ops, five failures and three cancellations. They contain 76
acknowledged physical picks, 2,871 public point/tangent stations and two preview
images. Sources include open/closed polylines, a mixed line/arc polycurve, kinked
NURBS, a closed cubic NURBS seam, tilted geometry and coordinates near 3e7.
Holdouts cover reversed sources and picks, nonadjacent picks, folded paths,
parallel supports, oversized radii and dynamic radius/Trim changes.

Both builds replay all 30 successful commands through prepared, explicit-source
and selected-parent entry paths, plus all 38 UI workflows. Checks compare Native
coefficients, domains, samples, metadata, IDs, selection, Undo/Redo and 3DM round
trips at a 1e-6 gate. The five API rejections separately verify complete document
immutability and pending Redo. A windowless mouse test exercises repeated source
selection and exact preview/commit geometry. Distinct-source regressions and
seven NURBS decomposition tests run alongside these checks.

```sh
python3 -m tools.rhino_oracle.check_fillet_self
```

Captures use private Xvfb displays; application tests create no windows. This is
sampled finite coverage. Fully consumed multi-span junctions, arbitrary smooth
self-bitangencies and all highly folded or self-intersecting sources remain
unqualified. There is no continuous-distance, pixel-equality or comparative
performance claim. Decomposition and junction search require work proportional
to the number of pieces; non-C0 NURBS cuts still need interval extraction.
