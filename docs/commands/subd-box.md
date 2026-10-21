# SubDBox

`SubDBox` creates a closed, smooth subdivision box on the active construction
plane. The rectangle defines its control cage; the rounded limit surface sits
inside that cage. The default is two faces along each box edge.

```text
SubDBox 0,0,0 4,3,0 5
SubDBox XCount=2 YCount=3 ZCount=1 Center 1,2,3 3,5,3 4
SubDBox 3Point 1,2,3 4,6,3 0,5,5 4
SubDBox Vertical 1,2,3 4,6,3 0,5,8 4
```

Start the command, set `XCount`, `YCount`, and `ZCount`, then pick or type the
base rectangle and height. At the first prompt, a positive integer sets all
three counts; `0` picks the CPlane origin. Counts persist between commands,
including cancellation. Numeric rectangle dimensions share Rectangle's getter.
After completing the base, type a signed height or pick a height point. Enter
uses the base's Y width: a `4 × 3` base gets height `3`.

`Center`, `3Point`, and `Vertical` use the shared rectangle constructions.
`3Point` can also be chosen after entering the first corner.
`AroundCurve` selects a curve or surface edge and then a center on it. The base
is perpendicular to the guide, which remains unchanged. Scripts use
`AroundCurve Curve=<id> center corner height`, or `Edge=<id>:<index>` for a
surface edge. Ordinary coordinate triples in scripts are world coordinates;
the interactive getter also accepts the shared CPlane, world, relative, unit,
constraint and snap input forms.

Base outlines and the height cage preview stay outside the document. Escape
cancels without consuming Redo. Invalid heights leave the getter active for
correction. Changed model history or tolerance invalidates a pending draft.
Completion adds one unselected SubD on the current layer with one Undo record.
The result supports existing SubD display, control-point edits and 3DM Save/Open.

The installed Rhino `8.32.26160.13001` rejects `Diagonal`, `Cube`, and
`EdgeMidpoint` for this command, although the
[online help](https://docs.mcneel.com/rhino/8/help/en-us/commands/subdbox.htm)
lists them. Viboceros rejects those options too. Six native diagnostic captures
retain the raw unknown-command results, including fallback geometry created by
remaining macro tokens.

The [qualification report](../subd-box-qualification.json) retains 23 captures:
16 successful constructions, cancellation, and six unsupported-option controls.
Command and viewport getter comparisons check control topology, winding, tags,
vertex limit stations, sampled density-two surfaces, source preservation,
attributes, selection and grouped history at `1e-6`. AroundCurve coverage uses
two straight guides; general curved guides and surface-edge frame parity need
further qualification. Sampled facets have no continuous Hausdorff certificate.
Counts must be positive and fit the 100,000-control-vertex construction budget;
comparative performance remains unqualified.
