# Line Tangent mouse input

A sliding Tangent uses the mouse point on the CPlane. An elevated closed curve
can therefore reject a free endpoint even when a tangent reaches the mouse's
viewing line. Enable Planar to construct parallel to the CPlane through the
contact, or FromFirstPoint to fix the contact and follow its tangent line.
A view parallel to the CPlane may provide no free endpoint at all.

[Validation](../line-tangent-rays-validation.json) retains 20 native UI
observations and two public SDK experiments. Eleven complete workflows match
in each build: three Planar/boundary constructions and eight no-output mouse
workflows followed by explicit cancellation. Seven reject endpoint coordinates; Front
has no CPlane intersection. Cancellation preserves pending redo; successful
lines survive 3DM round trips. Windowless mouse events check preview/commit,
relative input origins and unchanged source storage.

Eight pilot timeouts remain diagnostic inputs; their clean cancellation repeats
supply the rejection coverage. The earlier fixed perspective replay used
world-space viewing-line distance, which differs from screen-space picking. [Projected curve picking](../curve-picking.md)
resolves this calibration difference using public pre-click transforms; the
historical record and its diagnostic remain unchanged.

Six public zero-finder callback/bracket combinations converge for all 2,002
recorded seeds, while Rhino's tangent query fails on 30. This rules out those
experiments as an explanation of the earlier local-search mismatch; production
does not use them. General perspective picking, singular curves, additional
snap/Ortho/tracking combinations, continuous distance and speed remain unqualified.

Run `python3 -m tools.rhino_oracle.check_line_tangent_rays` for both builds and
shared regressions. Native capture generators run through `run_headless.sh`;
the check runner uses no desktop windows. Frozen producers and capture receipts
are retained with the validation evidence.
