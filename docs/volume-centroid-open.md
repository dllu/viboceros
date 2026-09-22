# Open-volume confirmation audit

[VolumeCentroid](commands/volume-centroid.md) · [Mass integration](mass-properties.md)

Native `VolumeCentroid` now asks before calculating open-object collections.
This audit retains the Rhino observations that motivated that workflow, including
real counterexamples. Open pieces are meaningful as volume boundaries only when
they jointly enclose a consistently oriented region. No performance parity is claimed.

The later [scalar/collection audit](volume-command-audit.md) also implements this
workflow for `Volume`. It establishes that Rhino's public collection API matches
the isolated-surface centroid command while the single-object API does not;
the cone first-moment discrepancy was unresolved at that stage. The later
[surface-primitive investigation](volume-surface-primitives.md) derives the
alternative density and adds 36 independent controls, without replacing this
historical capture or its original 38/42 comparison report.

## Dialog and owned input

The earlier [open-object request](../tools/rhino_oracle/fixtures/volume_centroid_open.json)
[timed out](../tools/rhino_oracle/observations/volume_centroid_open_timeout.txt)
without a command result. A follow-up run's progress markers and private-Xvfb
screenshot identified the modal `Rhino 8  Volume properties of non-Closed`
dialog, not an API calculation hang. It warns that the selected objects must
fully enclose a volume and offers Yes, No and Help. Yes is initially focused.

The new harness requires an explicit `open_confirmation` input: `yes`, `no`,
or `escape`. The last means the Escape key, **not a presumed cancellation**.
It validates the operation and progress marker, exact dialog title, owned PID,
and same-PID Rhino transient parent. It lets each window settle for 0.5 seconds,
rechecks ownership, activates only that verified dialog, and sends one key
sequence. Completed markers suppress stale input. Missing/unrelated windows
receive none; ambiguous ownership or input errors abort the capture. Existing
user Rhino sessions are never targeted.

Focused XTest input is intentional: a preliminary XSendEvent attempt lost an
early key and later failed when Escape destroyed its target before key-up.
That [terminal failure](../tools/rhino_oracle/observations/volume_centroid_confirmation_attempt.txt)
is retained separately and is not a completed observation. Tool stdout/stderr
are captured so X11 warnings cannot corrupt the response JSON.

## Observed behavior

The [source-only generator](../tools/rhino_oracle/references/volume_centroid_confirmation.py),
[42-case request](../tools/rhino_oracle/fixtures/volume_centroid_confirmation.json),
and [full observations](../tools/rhino_oracle/observations/volume_centroid_confirmation.json)
retain constructed/stored geometry, source and stored mass APIs, signed first
moments, command completion events, output points/attributes, selection,
history, and actual dialog-input diagnostics. Captures use Rhino
8.32.26160.13001 in a newly owned private-Xvfb session.

- The 30 open-input cases displayed the warning. Yes and Escape both continued;
  No returned unsuccessful completion without a marker. Escape's behavior is an
  observation of this warning, not a general Rhino cancellation rule.
- Six closed tetrahedron and six inconsistent-winding-but-closed mesh controls
  displayed no warning and received no input. The latter succeeded without a
  marker at zero signed volume. Topological closure and orientation are distinct.
- Preselection remained selected, including after No. Command-first completion
  cleared selection, including after No. Successful markers were unselected,
  unnamed, ungrouped, and on the current layer; source geometry was unchanged.
- Removing the tetrahedron's base gave API volume `5` and command centroid
  `(0.375,0.5,1.875)`. Translation by `(1e6,-2e6,3e6)` translated that centroid
  accordingly. These are open-input outputs, not a claimed enclosed solid.
- The warped open quad had signed API volume `-4` and first moments
  `(-8,-6,-2)`. The command placed `(2,1.5,0.5)` although the negative-volume API
  centroid getter returned the origin, as in the closed-mesh audit.
- The open bilinear surface's raw API had near-zero volume and enormous
  centroid coordinates, while the command placed approximately
  `(5/3,5/4,1)`. Command and raw API targets remain separate; neither is silently
  substituted for the other.
- Six unjoined mesh faces, or six unjoined NURBS surface faces, enclosing the
  `4 × 3 × 2` box placed its centroid `(2,1.5,1)` within `1e-9`. Their individual
  API volumes were zero or numerical residuals. This demonstrates why summing
  independently centered open-object properties is insufficient.

See [capture provenance](volume-centroid-open-provenance.json). The original
capture-only commit rejected native confirmation replay; the kernel and command
now implement that workflow, without changing the retained observations.

The [historical cone-only replay](volume-centroid-open-comparison.json) matched **38/42**
cases at absolute `1e-9`, relative zero. All 42 warning-presence, completion,
selection and marker-attribute checks agree; four isolated-surface marker
coordinates differ. The maximum coordinate error among matching cases is
`4.45e-16`. These results are separate from the unchanged 26 closed-command matches.

## Native boundary integration

`VolumeBoundary` gives meshes, B-reps and natural NURBS surfaces a shared kernel
interface. `VolumeMassProperties::from_boundaries` chooses the center of their
combined bounds; `volume_flux(base, ...)` accepts an explicit common reference.
For a surface point `S`, oriented normal density `n`, and `q = S - base`:

```text
V       = integral(q · n) / 3
M_world = base * V + integral(q * (q · n)) / 4
```

These cone-flux densities integrate a genuinely enclosing oriented collection
independently of the reference, and are well-defined but reference-dependent
for an isolated open piece. Mesh contributions are exact for stored binary64
coordinates and the selected binary64 base. Expanding determinants before
accumulation preserves cancellation without rounding `vertex - base`, even for
overflowing differences. B-rep contributions retain bounded quadrature on exact
surface/trim geometry. Strict per-solid mass APIs still reject open/nonoriented
inputs; explicit boundary APIs do not pretend to certify closure.

The isolated bilinear patch is a known command mismatch: native cone integration
at base `(2,1.5,1)` gives `V=-2` and centroid `(2,1.5,2/3)` from independent
polynomial integrals. Rhino's command produces `(5/3,5/4,1)`, and its raw API has
a different, near-zero-volume result. No targets are copied and no epsilon is
enlarged to make these conventions agree. All cases are replayed, including the
counterexamples; the existing 26 closed-command controls remain a separate scope.

UI tests distinguish the warning's Escape-as-Yes from cancelling selection or
replacing a pending command. No preserves preselection but clears postselection,
without changing geometry or history. Yes creates one undoable marker. The
scalar `Volume` command still has its older closed-input restriction; this
workflow currently belongs to `VolumeCentroid`.

McNeel's [public mesh mass-properties documentation](https://mcneel.github.io/rhino-cpp-api-docs/api/cpp/class_o_n___mesh.html)
also specifies a common base point for separately represented boundary pieces.

```sh
python3 -m tools.rhino_oracle.references.volume_centroid_confirmation > /tmp/volume-confirmation.json
tools/rhino_oracle/run_headless.sh rhino /tmp/volume-confirmation.json --timeout 300
python3 -m unittest tools.rhino_oracle.test_volume_confirmation
# Full native comparison, retaining isolated-surface differences (exit 1).
python3 -m tools.rhino_oracle.volume_centroid_replay \
  tools/rhino_oracle/fixtures/volume_centroid_confirmation.json \
  tools/rhino_oracle/observations/volume_centroid_confirmation.json
```
