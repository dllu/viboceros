# Chamfer end and seam selection

[Command syntax](commands/chamfer.md) · [Validation record](chamfer-branch-validation.json)

For nonmeeting open arcs and NURBS, Chamfer considers both ends and chooses the
least added continuation to the closed boundary. A pick near the far end can
therefore produce a continuation at the other end. Original parameter charts,
primitive types, source IDs, attributes, groups and history remain intact.

Intersections rank by unwrapped parameter distance on the closed sources.
The pick and corner parameters also choose each closed-source setback direction.
Moving a closed curve's seam can change the chosen intersection or direction,
following the captured native parameter-chart behavior.

Boundary-box distances order the extension candidates and skip an end that
cannot improve the known result. Fixed-accuracy probe bounds reject disjoint
curve boundaries before a very distant extrapolation enlarges the general
intersection search tolerance. Unresolved probe bounds use the original solver, and
delegated producer errors still propagate.

Four immutable licensed-Rhino command captures contain 46 workflows: 43
successes, two failures and one cancellation. Both builds compare all successes
through prepared and explicit-source APIs, plus all 46 UI workflows. Checks
cover coefficients, knots, domains, 6,138 original point/tangent stations,
metadata, selection, Undo/Redo and 90 3DM round trips per build, including two
windowless mouse picks, at 1e-6. Coverage includes reversed picks and sources,
circles, rectangles, ellipses, shifted seams and original crossing geometry.
Both API rejections preserve the complete document and pending Redo.

A separate public extension API capture records 27 controls across three rigid
orientations: 15 valid extensions, 12 null results and 27 unchanged inputs.
Those controls are consistent under the transforms. They narrow the earlier
mixed-style discrepancy to command-level selection; they do not qualify the
still-unsupported `ExtendArcsBy=Line`, `ExtendOtherCurvesBy=Smooth` arc branch.

```sh
python3 -m tools.rhino_oracle.check_chamfer_branch
```

Native captures use private Xvfb displays and isolated runtime directories.
Application tests create no windows. This finite corpus does not qualify every
continuation ranking, tied lengths, arbitrary folded or oscillatory branches,
all overlaps, continuous distance bounds, pixel equality or comparative Rhino
performance.
