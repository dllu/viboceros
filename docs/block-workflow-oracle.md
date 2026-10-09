# Block workflow oracle

The `block_workflow` Python/Rust protocol records embedded block creation,
insertion, shared redefinition and one-level or recursive explosion. It calls
document APIs locally and public RhinoCommon/RhinoScriptSyntax APIs in Rhino.
Command getter phases, option memory and viewport interactions are separate
contracts.

The October 8, 2026 private-Xvfb captures retain 44 workflows on Rhino
8.32.26160.13001: 20 command cases, 20 SDK cases and four root-text follow-ups.
All 22 command cases match the corrected local document behavior at absolute
`1e-9` plus relative `1e-12`. The main batch retains 85 states and 3,477 geometric
stations; its largest numeric discrepancy is `1.43e-14`.
See [source and capture provenance](block-workflow-provenance.json),
[command comparison](block-workflow-command-comparison.json),
[metadata comparison](block-workflow-metadata-comparison.json) and
[SDK comparison](block-workflow-sdk-comparison.json).

Use a private settings scheme and Xvfb for live captures:

```sh
tools/rhino_oracle/run_headless.sh rhino \
  tools/rhino_oracle/fixtures/block_workflow_commands.json \
  --scheme VibocerosOracleBlockCommands20261008 --timeout 300 \
  --output /tmp/block-workflow-rhino.json
python3 -m tools.rhino_oracle replay \
  tools/rhino_oracle/fixtures/block_workflow_commands.json \
  --observations /tmp/block-workflow-rhino.json
```

The host API is `OracleClient.run_rhino(request)` or
`OracleClient.run_viboceros(request)`. This operation requires `iterations: 1`.
Its `sources` use the existing object-source schema: points, point clouds,
meshes, curves, NURBS surfaces and supported trimmed B-reps. An optional parallel
`attributes` array supplies names, RGB colors, `color_source` (`layer`, `object`
or `parent`), `user_text` and `geometry_user_text`. Source objects use a private
Source layer; newly created root instances use a private Current layer. Both
layers are black. Insert steps may supply their own attributes.

| Action | Required fields | Effect |
| --- | --- | --- |
| `create` | `name`, `base`, `sources` | Capture live object handles in document order, normalize to the base, replace the sources with one instance; reuse of a name updates the shared definition. |
| `insert` | `name`, `transform` | Add an instance with an explicit row-major affine 4×4 matrix; reflections and shear are allowed. |
| `explode` | `object` | Expand one live instance; optional `recursive: true` expands nested instances to geometric leaves. `api` selects the native reference entrypoint: `command` (default) or `sdk`. |
| `group` | `objects` | Group live handles without creating a new object handle. |
| `explode_batch` | `objects` | Explode multiple roots recursively in one command; optional `group_output: true` creates one output group per root. |
| `rename_definition` | `name`, `new_name` | Rename a definition without replacing its ID or instances. |
| `delete_definition` | `name` | Delete a definition and all model roots; `expect_failure: true` records the manager's nested-definition restriction. |
| `make_unique` | `objects`, `name` | Rebind selected roots of one definition to one new duplicate, retaining object handles. |
| `duplicate_definition` | `name`, `new_name` | Create an unused definition copy without rebinding any model roots. |
| `replace_block` | `objects`, `name` | Replace roots of one definition; `all_instances: true` includes all model peers. Optional `replacement_instance` identifies a live target root using that named definition; Rhino receives it through its object getter. |
| `object_state` | `objects`, `mode` | Set owned model objects to `normal`, `hidden` or `locked` for protected-state probes. |
| `add_objects` | `target`, `objects` | Append live sources in a target instance's local frame, refresh its definition and consume the sources. The target and explicit sources must be editable; graph cycles reject before native host access. |

Group captures use `record_groups: true` to include ordered object/prototype/leaf
memberships and the group table's model-handle membership. Create steps may choose
`api: command` for a new plain-token name; otherwise they use SDK construction.
Recursive command explosions support explicit `group_output: true` or `false`.
See the [group oracle](block-groups.md) for those construction/API differences.
`record_management: true` adds top-level/nested/total model-use counts and active
catalog-reference counts. The [BlockManager guide](commands/block-manager.md)
describes the management contract and native reference policy.
`record_states: true` adds each live handle's visibility/locking state.
See the [ReplaceBlock guide](commands/replace-block.md) for the replacement contract.

Handles start at zero in source order. Every output receives the next handle;
deleted handles are never reused. Each state lists the live objects, the whole
owned definition catalog and, after a step, its output handles. Instance records
contain the definition name, transform and placed geometric leaves with ordered
`[definition name, member index]` paths. Definition members retain local geometry
or nested references. Attributes and geometry text are recorded separately.

Curves have 33 parameter stations, surfaces have 25 chart stations, meshes have
their vertices, and point clouds retain point order. The B-rep adapter also
records face charts and trim witnesses. These samples can reveal placement and
parameterization discrepancies; they do not prove continuous geometric agreement.

The native placed-member witness converts an ArcCurve to its exact NURBS
representation before applying a nonsimilarity matrix. Direct ArcCurve.Transform
cannot represent an arbitrary affine conic and is unsuitable as that witness.
Native single-surface B-reps are recorded as surfaces, and geometry strings are
attached to the admitted native object after surface wrapping.

The local operation always uses the document kernel. Choosing `sdk` changes
the native reference entrypoint, so API-specific metadata and representation
differences remain diagnostics rather than silently changing local command
behavior. The SDK resolves ByParent colors to explicit RGB; native commands
retain raw ByParent attributes. With an empty root attribute-text collection,
the retained SDK cases also drop member attribute text. With root text present,
the four-case follow-up retains member text and omits root-only keys. Circle
conversion and parameterization differences are retained in the SDK records.
The SDK comparison deliberately reports 18 mismatches in its main 20-case batch;
the metadata follow-up reports two additional SDK color-policy mismatches.
Those comparison runs exit with status 1 and retain no native execution failures.
The original SDK capture and its frozen helper are preliminary diagnostics from
before the surface/placed-conic recording corrections.

Validation bounds a workflow to 32 sources, 64 steps, 4,096 lifetime handles and
65,536 geometry records. The native validator resolves handle lifetimes, graph
cycles/depth and expansion budgets before touching the host. Native objects,
layers and definitions use a private namespace and are removed after each case.
An unrelated document's selection and current layer are restored.

These cases cover embedded definitions and supported geometry/metadata only.
The earlier capture excludes group membership; the follow-up linked above covers
that scope. Hidden/locked source policies, general command getters, option memory,
linked definitions, subobject editing, broad B-rep source normalization and
performance remain outside this capture. No kernel timing is collected.

API references: [instance definitions](https://developer.rhino3d.com/api/rhinocommon/rhino.docobjects.tables.instancedefinitiontable),
[RhinoScriptSyntax block methods](https://developer.rhino3d.com/en/api/RhinoScriptSyntax/#block).
