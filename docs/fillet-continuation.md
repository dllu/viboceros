# Fillet continuation beside closed sources

[Command reference](commands/fillet.md) · [Validation record](fillet-continuation-validation.json)

A distinct open curved partner can continue to a fillet contact beside a closed
planar source. `ExtendArcsBy=Arc` uses the supporting circle; `Line` uses an endpoint
tangent. `ExtendOtherCurvesBy=Line|Smooth` selects endpoint tangents or the continued
NURBS endpoint chart. Only the required continuation enters the document. Closed
parents remain complete, and the existing trim/join rules preserve identity,
attributes, groups, selection and atomic Undo/Redo.

Circular continuation preserves native parameter speed while finding contacts;
extended retained arcs then reset their domain to arc length. Tangent segments use
the original endpoint derivative speed. Smooth continuation uses the selected
portable/usd-brep extension backend. It searches expanding endpoint charts and ranks
the contact branches it finds by summed arc lengths from the original picks,
consistent with the public
[GetFilletPoints contract](https://developer.rhino3d.com/api/rhinocommon/type/rhino.geometry.curve/).
Length bounds and polynomial Taylor-sign certificates can exclude unseen tails.

The closest contact can produce a minor arc whose tangent opposes the retained
open piece. Native command controls accept that arc with `Trim=No`, but fail when
trimming, including with `Join=No`. Admission checks the selected contact's tangent
without substituting a farther branch. A missing contact can instead be a successful
no-op; both outcomes preserve geometry and pending Redo.

Four immutable licensed-Rhino captures contain 44 complete workflows: 31 edits,
four successful no-ops, eight failures and one dynamic cancellation. They include
circular, tangent-line and quadratic smooth continuation; reversed picks and source
directions; rigid placement in space; radius changes; shorter and longer source
charts; and disjoint supports. There are 88 acknowledged physical picks and 6,171
public point/tangent stations. Authored geometry and pre-command sight points supply
inputs; native output coefficients, domains and samples serve only as expectations.

Both builds replay all command outcomes through the UI, all 35 successful explicit
commands, and eight pure API rejections. Checks include metadata, source identity,
history and 80 3DM round trips per build. A separate windowless mouse test verifies
extension preview reuse, exact committed geometry and a single history entry.
Eight public SDK controls separately audit continuation contacts, tangent direction
and unchanged inputs. An initial observer used the unavailable `Arc.AngleRadians`
property; its failed log and frozen source closure remain archived with zero
observations. The corrected observer uses `Arc.Angle`. A successful command capture's
recipe-spacing SyntaxWarning is also retained explicitly.

```sh
python3 -m tools.rhino_oracle.check_fillet_continuation
```

Native captures run on private Xvfb displays; replay and egui checks open no windows.
The model gate is 1e-6. Coverage is finite: multistart search does not prove all roots
were found, and smooth continuation stops after at most 40 domain expansions.
Arbitrary folded or oscillatory branches, tied costs, rational poles, stationary
tangents, nonplanar contacts and corner contacts remain unqualified. These checks
make no continuous-distance, pixel-equality or comparative-performance claim.
