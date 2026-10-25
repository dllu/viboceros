# Interface state API

The `interface_commands` operation records the initial state and the state after
each bounded interface command. Supply `subd_control_net` to initialize and record
SubD appearance: `true` means flat control net and `false` means smooth surface.
SubD display commands require this explicit initial value. Nulls, numbers and
strings are rejected. Requests that omit the field retain their existing output
schema.

```json
{
  "protocol_version": 1,
  "iterations": 1,
  "operations": [{
    "op": "interface_commands",
    "id": "display-toggle",
    "grid_snap": true,
    "osnap": true,
    "smart_track": false,
    "active_viewport": 0,
    "display_modes": ["Wireframe", "Shaded", "Ghosted", "Wireframe"],
    "subd_control_net": false,
    "commands": ["SubDDisplayToggle", "Snap", "SubDDisplayToggle"]
  }]
}
```

The SubD states in this example are `[false, true, true, false]`. Viewport shading
and SubD appearance are independent. Prefixes such as `'_SubDDisplayToggle` are
accepted; extra command tokens are rejected.

Run `python3 -m tools.rhino_oracle compare REQUEST.json` to compare both engines.
Live Rhino tests use the private headless wrapper described in the
[oracle guide](../oracle.md). The worker restores full drafting and tracking
snapshots, viewport modes, the active view, and requested mesh/SubD settings.
Every restoration is attempted even when another cleanup action fails; the first
cleanup failure is reported.

The [qualification](../interface-subd-qualification.json) compares 64 native
sequences with the Rust API and GUI, covering both initial appearances, all eight
basic switch combinations, all four active viewports, and 640 states per path.
