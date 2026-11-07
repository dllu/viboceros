# Ordered Fillet input and dynamic radius preview

[Command reference](commands/fillet.md) · [Validation record](fillet-getter-validation.json)

`Fillet` now picks its sources in order. `Dynamic=Yes` opens a radius getter after
the second pick. Numbers and two-point distances update the preview and keep the
getter active; Enter accepts. The preview keeps both original curves highlighted
and draws prospective pieces separately. It creates no model objects or history
entries. Candidate geometry is reused when an accepted distance matches the hover,
and the same prepared geometry commits in one atomic edit.

For two open curves, trimmed sources keep their IDs and attributes. Joining retains the first picked
source; a separate arc gets fresh attributes on the current layer. Results are
deselected. Retained lines keep their source parameter charts, and joined curves
retain the oriented first source's domain origin. The shared portable/usd-brep
fillet producers still own contact geometry and curve extension. The native
contact adapter now restores the new arc's physical arc-length chart after
scaling it from its unit-radius working frame. Derivative checks exercise this
contract at model scales from 1e-6 through 1e6.

Radius and output/extension choices survive cancellation, while Dynamic resets
for the next invocation. Join is unavailable with Trim=No; its remembered value
is retained. Replays include two Native rejections of an unavailable Join
assignment followed by successful continuation. Zero-radius, already meeting
sources with Join=No can complete without creating a history entry.

Four immutable licensed-Rhino captures contain 29 observations. The 25 complete
workflows comprise 16 successes (14 edits and two no-ops), nine cancellations,
41 acknowledged physical picks, 3,300 independent point/tangent stations and eight
viewport images. The four driver diagnostics are retained separately: two pilots
omitted a radius value, and two dynamic cases omitted the final Enter. Corrected
recipes subsequently completed.

A dispatch failure published no observations. A later verification family reached
an overlapping curved-source selection menu; manual dismissal/cancellation ended
that capture with an IndexError and no published observations. Its frozen producer
closure, request, log and private-display inspection are retained. The entire
family was repeated, with separated picks for its curved case.

Both builds replay all 25 getters and the 16 successful explicit commands, checking
coefficients, domains, independent Native evaluations, attributes, identities,
selection, undo/redo and 3DM round trips. The model gate remains 1e-6. Separate
windowless mouse tests check viewport pick routing, cache reuse, persistent defaults and painting in four views
across Wireframe, Shaded and Ghosted modes.

```sh
python3 -m tools.rhino_oracle.check_fillet_getter
```

Rhino captures run on private Xvfb displays; application tests are windowless.
Saved pixels establish the observed composition, without a pixel-equality claim.
Closed sources have [separate coverage](fillet-closed.md). Self-source fillets, arbitrary branch choices, complete invalid-input
trajectories, alternate Native display settings, continuous-distance bounds and
comparative performance remain unqualified. Existing rational-extension and
fully consumed added-segment limits remain. A changed preview constructs a
temporary scene with work proportional to document size; unchanged candidates
reuse it.
