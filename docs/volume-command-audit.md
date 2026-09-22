# Scalar Volume and collection-API audit

[Volume](commands/volume.md) · [Open-centroid audit](volume-centroid-open.md)

The [source-only generator](../tools/rhino_oracle/references/volume_command.py)
constructs 80 cases: 74 actual `Volume` commands and six paired
`VolumeCentroid` controls. The [request](../tools/rhino_oracle/fixtures/volume_command.json)
and [complete observations](../tools/rhino_oracle/observations/volume_command.json)
come from Rhino 8.32.26160.13001 in an owned private-Xvfb session. Existing user
sessions were untouched. Every command must produce its public EndCommand event;
RunScript return values alone are not considered completion.

The capture covers all earlier closed/warped-mesh and warning fixtures, then
pairs both commands on six variants of the bilinear patch: original, translated,
cyclic axis permutation, axis reflection, anisotropic scale and reversed U.
Its source generator reads no captured targets. Constructed/stored definitions,
verified insertion reversal, source preservation, single-object APIs, explicit
tolerance APIs, collection APIs, command history, output points and selection
remain separate evidence. Numeric values in the compact retained JSON were
verified bit-for-bit against the original response.
See [capture provenance and hashes](volume-command-provenance.json).

## New observations

Scalar `Volume` has the same conditional warning and pre/postselection behavior
as the earlier centroid capture. Inconsistent-winding but topologically closed
meshes do not warn. No declines; Yes and Escape at the modal warning continue.
The owned-dialog driver verifies title, PID and transient parent and sends no
input to closed controls. The command creates no geometry.
An unsupported-only preselected line remains selected after failed scalar
Volume, unlike the earlier VolumeCentroid observation. Native failure cleanup
keeps this distinction instead of applying centroid cleanup to both commands.

Six unjoined mesh or surface faces enclosing the 4 × 3 × 2 box report volume 24,
even though their separate single-object API volumes vanish. The bilinear patch
`S(u,v)=(4u,3v,2uv)` reports −2, matching native common-base volume integration.

The public [collection Compute overload](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/M_Rhino_Geometry_VolumeMassProperties_Compute_7.htm)
reproduces the bilinear patch's actual centroid-command point `(5/3,5/4,1)`.
Its collection first moments are approximately `(-10/3,-5/2,-2)` at volume −2.
Translation, axis cycling, reflection, scaling and U reversal reproduce that
agreement between the collection API and actual command. By contrast, the
single-object API gives nearly zero volume and a huge centroid for the original
patch. Thus a single-object API result is not a valid substitute for this
command's result, even for a one-object selection.

This narrows but does not resolve native centroid compatibility: the explicit
cone-flux density has mathematically derived first moments `(-4,-3,-4/3)`, hence
centroid `(2,3/2,2/3)`. The collection API evidently uses a different open-surface
first-moment convention. Its implementation has not been inferred from these
examples, and no case-specific correction has been introduced.

## Comparison scope

The replay compares actual command values/points, warning presence, completion,
selection and marker attributes. Scalar targets come only from the controlled
English command-history result line, with observed document display precision
seven and millimeter model units. Collection API diagnostics are validated but
never used as command targets. Printed uncertainty does not widen the fixed
`1e-9` absolute / zero relative tolerance.

The [complete native replay](volume-command-comparison.json) matches **71/80**
cases: **71/74 scalar commands**, including all 42 original warning controls.
All 80 completion, selection and marker-attribute checks agree; warning presence
agrees in all 54 cases with explicit confirmation diagnostics.
The nine retained numerical differences are:

- Two outward curved-solid printed volumes differ by `4.54e-9` and `1.58e-9`.
- The inward capped B-rep has opposite sign after Rhino's insertion reversal.
- All six open-surface centroid controls retain the first-moment discrepancy.

The earlier independent centroid replays remain 26/26 closed and 38/42 warning
cases. No capture or comparison epsilon was changed to obtain these counts.

Curved command results have finite displayed precision and integration error.
The inward capped B-rep also reverses on Rhino document insertion, unlike native
insertion. Both effects are retained as differences, not corrected in the
fixture, hidden by an epsilon change, or treated as geometry-kernel targets.
SubD, the postselection Units option and Rhino performance parity remain untested
or unimplemented. Local analytic tests cover full-range exact mesh cancellation,
surface cone volume, closed box volume, UI input, and preserved model history.

## Local performance cost

The [median-of-seven release check](volume-command-performance.json) includes
one mesh closure inspection in both scalar paths and excludes construction.
For 6,144 quads, exact collection volume took 2.69 ms at the origin and 3.61 ms
at translation `1e12`; the older closure-checked floating scalar path took
2.51 ms and 2.83 ms. Full first moments took 5.75 ms and 14.43 ms respectively.
Thus skipping first moments avoids a large unnecessary cost, but exact scalar
accumulation is still slower than the older rounded calculation in this check.
These are kernel queries, not full command/UI latency or Rhino timings.
