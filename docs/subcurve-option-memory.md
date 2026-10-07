# SubCrv option memory

[SubCrv](commands/subcurve.md) · [RememberCopyOptions](commands/remember-copy-options.md)

SubCrv independently remembers Copy, Mode and FromMidpoint for the application
session. Initial values are No, Shorten and No. Accepted option changes take
effect immediately and survive cancellation. Omitted options use these saved
values in both the GUI and complete registry invocations. Invalid syntax does
not accept a new value. Document Undo/Redo does not restore previous preferences.

The captured SubCrv Copy choice is independent of the global RememberCopyOptions
switch: turning remembering off does not reset it when SubCrv starts. Other
commands retain their existing shared Copy policy. Separate registries have
independent SubCrv preferences; one registry shares them across its documents.
The values are not yet persisted across application restarts.

The [closed workflow](../tools/rhino_oracle/fixtures/subcurve_preferences.json)
ran on private Xvfb in `VibocerosOracleSubcurvePreferenceFinal20261007`.
[Raw records](../tools/rhino_oracle/observations/subcurve_preferences.json)
retain all 37 steps, including cancelled default queries, option edits, completed
geometry, global toggles and geometry snapshots. Copy=Yes hides Mode in some
native prompts; records preserve that absence rather than filling an assumed
value. Additional Copy=No queries and completed MarkEnds commands establish Mode
memory independently. Application replay compares every visible default and
every resulting locus or marker at `1e-6`, with identity and selection checks.
Command tests cover omitted values, malformed syntax, registry isolation and
history independence. See [provenance](subcurve-option-memory-provenance.json).

This does not establish restart persistence, direction-option lifetime, B-rep
edge preferences, or every failure-path timing in native getters. Source editing
and output metadata keep the existing command policies.

```sh
cargo test -p viboceros-command --release subcurve_preferences
cargo test --release --bin viboceros subcurve_preferences
```
