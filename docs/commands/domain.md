# Domain

[Command index](README.md)

Select one curve or surface and enter `Domain` to report its stored parameter
intervals. With no preselection, the application opens an object-selection prompt;
pick one eligible object and press Enter. Points, point clouds, and meshes are
not eligible. Multiple selected objects are rejected.

Curves report one interval; surfaces report U and V intervals. All native curve
families use their own domains, without converting to NURBS or using curve length
as a substitute. Reparameterized and imported domains are retained in the report.
No geometry, attributes, selection, or model undo/redo is changed by evaluation.

For a multi-face B-rep, the application then asks for a component-surface location.
`Domain 0.5,0.5,10` performs the same query at a world-space location. The nearest
trimmed face supplies its underlying surface's U/V domains; no aggregate domain is
invented for the polysurface. A single-face B-rep needs no additional pick.

For deterministic scripts, `Domain Face=0` reports a zero-based B-rep face index.
Face order is the current document topology order, not a persistent face identifier.
Out-of-range indices and unsupported options are errors. Example output:

```text
Curve domain = [-2,8]
Face 1: U domain = [5,6]; V domain = [-8,-3]
```

For a selected curve, `Domain SubCrv Parameter=0,6` reports the domain of the
directed interval from parameter 0 to 6. `Domain SubCrv 2,0,0 8,0,0` picks its
ends by closest points on the curve. A decreasing interval on an open curve
reverses the extracted segment, so its reported domain may differ from the
original parameter values. A decreasing interval on a closed curve crosses
the seam. Enter `Domain SubCrv` to pick the curve if needed and then pick its
start and end in the viewport or type them at the point prompts. An invalid end
leaves the prompt open for correction. These queries do not create an object or
change the selection.

The component-point phase preserves the accepted object selection. Esc cancels
that phase without model edits; failed point evaluations stay open for correction.

[Rhino's Domain command](https://docs.mcneel.com/rhino/8/help/en-us/commands/domain.htm)
also offers `SubCrv` picking. `Face=index` and the scripted `SubCrv` arguments are native scripting
conveniences, not claims of identical Rhino command syntax. Component lookup
currently uses 3D nearest-face distance, not a screen-space hit aperture.

Native tests cover stored/reparameterized curve intervals, independently
reparameterized component surfaces, explicit and picked faces, invalid selection,
redo retention, directed subcurves, and both preselected and postselected UI
transitions.

A [live Rhino 8.32.26160.13001 reference](../domain-rhino-reference.json) captures
actual `Domain` command output for a reparameterized line `[-2,8]`, a rational arc
`[7,11]`, an untrimmed surface with U `[-2,4]` / V `[10,20]`, and a single-face
B-rep with U `[5,6]` / V `[-8,-3]`. The native regressions agree with these
intervals. The capture also records public-API domains and confirms the source
geometry checksum is unchanged. It does not establish command-picking parity
for multi-face or trimmed B-reps, or for `SubCrv`.

The same fixture now includes forward, reversed, and closed-seam subcurve
cases. The worker records the RhinoCommon extracted curve's domain, endpoints,
and a `Domain` command report on that temporary curve. Those cases do not yet
have a live Rhino capture: the September 2026 headless attempt exited during
Wine startup in `dotnetstart` before the worker loaded. The native closed NURBS
regression checks its directed domain and sampled positions across the seam.

```sh
tools/rhino_oracle/run_headless.sh rhino tools/rhino_oracle/fixtures/domain-command.json --timeout 240
```

The probe owns and deletes its temporary source, disposes of temporary geometry,
and restores prior selection. Mock tests cover construction/capture failures and
reject stale history, incomplete surface reports, and unknown-command warnings.
Zero elapsed times in these captures are not performance measurements.
