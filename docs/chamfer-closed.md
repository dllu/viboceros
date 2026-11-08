# Chamfer with distinct closed sources

[Command syntax](commands/chamfer.md) · [Validation record](chamfer-closed-validation.json)

Closed parents remain intact. Trim replaces only an open source. Without Join,
the bevel is unnamed, ungrouped geometry on the current layer. Join replaces the
first source in place when it is open; otherwise it creates a fresh, ungrouped
result with that closed source's attributes. Open second sources are removed
when joined. Source IDs, attributes, groups, parameter charts and history follow
the same shared contact-admission plan used by Fillet.

Portable/usd-brep intersection events supply point contacts and overlap
boundaries. Picks rank the intersection and select each local direction. Closed
parents move their seam temporarily for each endpoint sphere query; the original
geometry is untouched. Infinite line supports are clipped against the other
source's bounds, preserving the original line chart. Euclidean setbacks also
work on bounded nonlinear NURBS. Zero distances can trim an open source without
adding a bevel; zero distances without trimming are a successful no-op.

Three immutable licensed-Rhino captures contain 33 workflows: 30 native
successes, two failures and one cancellation. Two successes create invalid
polycurves: zero distances between two closed sources with Join, and an oversized
joined cut consuming the open line. Their public validity, domain and endpoint
diagnostics are preserved. Viboceros rejects both without edits or Redo loss;
they are excluded from geometry parity.

Both builds replay the 28 valid successes through prepared and explicit-source
APIs, plus 31 complete UI workflows. Four API rejections verify full document
immutability. Checks compare coefficients, knots, 4,950 original public
point/tangent stations, metadata, IDs, selection, Undo/Redo and 60 3DM round trips
per build, including the windowless mouse test, at 1e-6. Cases include circles,
closed rectangles, rational circles, ellipses, nonlinear open NURBS, tangent and
coincident contacts, line extension, reversed picks/orientation and tilted data.
The coincident case uses public transparent SelID selection to avoid inherently
ambiguous mouse hits, with authored natural-end seeds.

Three unsuccessful capture attempts published no observations. Their requests,
producer snapshots and logs remain available. A perspective selection menu was
resolved by repeating the same geometry/picks in Top view. A native invalid
curve required a validity-aware observer. Typed SelID required correcting the
first/second curve prompt gate. Owned processes were verified before termination.

```sh
python3 -m tools.rhino_oracle.check_chamfer_closed
```

All captures use private Xvfb displays; application tests create no windows.
This finite corpus does not qualify arbitrary folded or oscillatory branches,
all overlap choices, continuous distance bounds, pixel equality or comparative
performance. Nonmeeting open arc/NURBS extensions have a
[separate qualification](chamfer-extension.md). Shared Chamfer/Fillet regressions, both workspace
Clippy configurations, formatting and Python/SubD qualification checks accompany
this suite.
