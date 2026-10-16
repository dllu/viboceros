# Flow implementation work

Flow is not registered as an application command yet. The current milestone
provides [shared prepared length queries](prepared-curve-length.md) and native
captures for implementing its curve-to-curve mapping.

The retained `tools/rhino_oracle/fixtures/flow.json` recipes provide 44 public
`FlowSpaceMorph.MorphPoint` captures from Rhino 8.32.26160.13001. Authored line,
arc, circle, polyline and spatial cubic guides cover both stretch modes,
reversals, source and target endpoints, off-guide points and tangent extensions.
Four recipes additionally run the actual Flow command on grouped point clouds.
Those four use Copy=No, Rigid=No, Stretch=Yes, Roadlike=No and
PreserveStructure=No. Their geometry agrees with their SDK witnesses, their
source selection and group membership remain, and their guides remain unchanged
and unselected.

The separate `flow_frames.json` capture contains 18 diagnostics: all straight
coordinate-axis source/target pairs, public curve frame queries, a translated
spatial guide, authored perpendicular probes and a dense spatial target sweep.
Returned native frames are observations, not construction inputs for the Rust
implementation.

Native experiments show that source points beyond an endpoint retain their
along-tangent residual separately from the mapped guide distance. The residual
uses the target tangent at that mapped distance; stretching also scales this
residual. Spatial frame orientation and length lookup still differ from the
unfinished local point-map prototype. These cases remain open at the intended
`1e-6` comparison threshold; there is no Flow parity or performance claim.

Remaining work includes resolving every retained point-map case, connecting
whole objects and selected grips through staged edits, curve picking near the
matching ends, Line and Local input, Roadlike and TargetSurface orientation,
Rigid/RigidGroups policies, Copy and PreserveStructure, previews, option memory
and history. Capture additional actual command workflows for these options
before qualifying their native behavior.

Capture the native baselines with an owned private Rhino instance:

```sh
python3 -m tools.rhino_oracle rhino tools/rhino_oracle/fixtures/flow.json --scheme VibocerosOracleFlowCapture --timeout 240 --output /tmp/flow-rhino.json
python3 -m tools.rhino_oracle rhino tools/rhino_oracle/fixtures/flow_frames.json --scheme VibocerosOracleFlowFramesCapture --timeout 240 --output /tmp/flow-frames-rhino.json
python3 -m unittest tools.rhino_oracle.test_flow_capture
```

References: [Rhino Flow command](https://docs.mcneel.com/rhino/8/help/en-us/commands/flow.htm)
and [public FlowSpaceMorph constructor](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/M_Rhino_Geometry_Morphs_FlowSpaceMorph__ctor_1.htm).
The capture uses public constructors, point mapping, curve queries and command
macros without inspecting proprietary implementation code.
