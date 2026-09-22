# Open-volume confirmation audit

[VolumeCentroid](commands/volume-centroid.md) · [Mass integration](mass-properties.md)

Native `VolumeCentroid` still requires solid, consistently oriented input.
This audit captures Rhino's actual command and warning behavior before extending
that contract. No open-volume native compatibility or performance is claimed.

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

These results constrain a future shared-reference integration/confirmation
workflow, but do not establish a general open-surface reference-frame policy.
No tolerance was enlarged to hide the API/command differences. Both the Python
replay entry point and the native Rust runner reject confirmation fixtures
explicitly until native support exists; none of these 42 cases is counted among
the existing 26 closed-command matches. See [provenance](volume-centroid-open-provenance.json).

```sh
python3 -m tools.rhino_oracle.references.volume_centroid_confirmation > /tmp/volume-confirmation.json
tools/rhino_oracle/run_headless.sh rhino /tmp/volume-confirmation.json --timeout 300
python3 -m unittest tools.rhino_oracle.test_volume_confirmation
```
