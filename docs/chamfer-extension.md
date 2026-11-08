# Chamfer with nonmeeting curved sources

[Command syntax](commands/chamfer.md) · [Validation record](chamfer-extension-validation.json)

An open arc or NURBS can extend to a distinct closed curve before the setbacks
are measured. Circular continuation retains the arc primitive. Tangent lines
retain the original endpoint parameter speed, including on NURBS. Smooth
continuation uses the existing portable/usd-brep boundary-extension producer.
A circular cut outside the original source resets to a length domain; a cut
inside retains its original parameter chart. The closed parent remains whole.
Trim, Join, source attributes, groups and IDs use the shared Fillet/Chamfer
contact-admission plan.

Four immutable licensed-Rhino captures contain 37 workflows: 35 successes, one
failure and one cancellation. Both builds compare 29 supported successes through
prepared and explicit-source APIs, and 31 complete UI workflows. Coverage
includes circular, tangent and smooth continuation, reversed picks and source
orientation, cuts crossing into the original source, and tilted geometry.
Verification controls explicitly set both extension styles.

The corpus records 5,148 native point/tangent stations, including 4,356 in the
supported workflows. Checks compare coefficients, knots, domains, these stations,
metadata, IDs, selection, Undo/Redo and 61 3DM round trips per build, including
two windowless mouse picks, at 1e-6. Seven API rejections preserve the complete
document and pending Redo.

Six valid native successes remain unqualified: nonmeeting open arcs with
`ExtendArcsBy=Line` and `ExtendOtherCurvesBy=Smooth` produced different
continuation paths under rigid transforms. Their observations remain in the
corpus. Viboceros rejects this combination against a closed source without an
edit; use `ExtendOtherCurvesBy=Line` for tangent arc continuation.

```sh
python3 -m tools.rhino_oracle.check_chamfer_extension
```

Native captures use private Xvfb displays and isolated runtime directories;
application tests create no windows. This finite corpus uses circular closed
boundaries. Arbitrary boundaries, folded or oscillatory branches, all
intersection choices, continuous distance bounds, pixel equality and
comparative performance remain unqualified.
