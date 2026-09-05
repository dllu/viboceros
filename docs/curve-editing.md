# Curve joining and closure

[Curve commands](commands/curves.md) · [Exact polycurves](polycurves.md)

`Curve3` owns the seven curve representations shared by `CurveRef`. Editing may
retain a representation or return a polycurve; it never fits an approximating
curve. Endpoint replacement clamps nontrivially edited NURBS curves to their
active domain, preserves rational endpoint weights, and rebuilds bounds.
Exact no-op edits do not clamp or alter the original control structure.
The public OpenNURBS implementations of `SetStartPoint`, `SetEndPoint`, and
`ON_ForceMatchCurveEnds` were also inspected as compatibility references;
the Rust circle reconstruction uses the endpoint/chord/tangent geometry directly.

## Joining

The kernel exposes two explicitly different `CurveJoinStyle` policies:

- `Batch` matches nearest compatible endpoints, using tangent alignment to break
  distance ties. It forms independent chains, prefers the majority of original
  directions, and favors the last source on a direction tie. Linear outputs are
  chord-length-parameterized polylines, retaining intermediate vertices.
- `Seeded`, used by `Join`, extends the earliest unused source in one pass through
  later inputs. An earlier skipped source is not revisited after a later extension.
  The seed's direction and original parameter interval are retained, including
  negative parameters when another curve is prepended. Linear inputs preserve
  their individual vertex parameters.

These differences were observed separately in Rhino's public `JoinCurves` API
and interactive command. Endpoints of two flexible curves move to their midpoint.
An analytic arc stays fixed against a flexible curve. Two analytic arcs meet at
the midpoint, each retaining its opposite endpoint and tangent through exact
supporting-circle reconstruction. Mixed outputs retain exact NURBS segments and
original interval widths; endpoint edits can change physical length without
changing those intervals.

The command creates new joined object IDs, inherits the earliest source's
attributes and group memberships, deletes consumed inputs, and leaves disconnected
inputs unchanged. It stages validation and records one atomic undo step.
Preselection uses the document's recorded selection order, not UUID ordering.

Positive tolerances use spatial endpoint buckets. Exact-zero tolerance uses exact
coordinate keys; extreme coordinates use a widest-axis sweep. Limits of 100,000
inputs, one million candidate pairs, and 16 million matching scans bound resource
use. Reaching a limit reports an error rather than producing a partial edit.
The older unambiguous `join_polylines` utility remains for topology algorithms
that explicitly require branch rejection; it is not the interactive Join policy.

## Closure

`CloseCrv` accepts `Tolerance=value` and `CloseWideGapsWithLine=Yes|No`.
Eligible NURBS/polyline/polycurve endpoints within the tolerance move to the start.
If that cannot make a valid closed curve, an allowed closing line is appended as
a separate polycurve segment—even for a short gap or an input polyline.
The new segment's interval width is its length. Original intervals are retained.
Straight curves and already-closed objects are unchanged.

Zero tolerance forces eligible flexible endpoint edits. For analytic arcs it
completes the supporting circle while retaining the old arc interval. Positive
tolerance does not apply that special arc rule: an allowed line closes the gap.
Closure keeps object IDs, attributes, groups, selection, and atomic undo/redo.

## Evidence and remaining limits

The 39-case `curve_join_close.json` fixture was compared with Rhino
8.32.26160.13001 using absolute epsilon `1e-8` and relative epsilon `1e-10`;
observed maximum numeric difference was below `1.6e-12`. This covers rational
definitions, domains, orientation, short/wide closure, branch and shuffled input,
two-arc matching, native polyline parameters, and command identity/group behavior.
Unit tests additionally check unclamped and weighted endpoint edits, bounds,
resource guards, exact-zero matching, parameterized evaluation, and history.
3DM tests distinguish native parameterized polylines from degree-one NURBS.

This is bounded compatibility evidence, not full equivalence for arbitrary curves.
Polycurve segments are currently stored in NURBS form, so an imported or previously
joined analytic arc no longer carries its original analytic endpoint-edit policy.
Rhino's full simplification, high-degree polyline recognition, tiny-segment removal,
and all ambiguous/degenerate join cases remain incomplete. Higher-level joining of
surfaces, B-reps, and meshes is not implemented by this command.
Probe timings include recording and command/document work and are not a
kernel-only performance comparison.
