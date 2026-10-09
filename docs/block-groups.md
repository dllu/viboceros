# Block groups

The October 8, 2026 follow-up records 16 complete native command workflows and
14 SDK-construction workflows on Rhino 8.32.26160.13001, running under private
Xvfb. The earlier implementation matched none of the 16 command cases. The
corrected creation/explosion policies match all 16 cases and 82 states: geometry,
definition relationships, metadata, ordered memberships and group tables.
See [source/capture provenance](block-group-provenance.json).

| Operation | Group behavior |
| --- | --- |
| Block | Copy each source's top membership into a fresh definition group. Source objects with overlapping memberships retain only the top one inside the definition. |
| Explode | Copy direct prototype top groups into fresh model groups for each root. |
| ExplodeBlock | Retain existing leaf groups; discard groups on nested reference containers. Repeated placements or sequential explosions can join the same leaf group. |
| Root memberships | Discard during either explosion. Untouched model peers keep their memberships. |
| GroupOutput=Yes | Append a fresh output group per root, including a multi-root command. |

The published [Block grouping rule](https://docs.mcneel.com/rhino/8/help/en-us/commands/block.htm#Groups)
says grouping remains inside blocks. Native command construction makes fresh
prototype groups; direct InstanceDefinitionTable.Add strips the supplied group
indices. The SDK-construction dataset retains that difference rather than being
used to redefine the command contract.
The SDK-construction replay matches three cases and retains eleven group-policy
diagnostics with no native execution failures. Its diagnostic run exits with
status 1. [Command results](block-groups-command-comparison.json),
[SDK results](block-groups-sdk-comparison.json) and
[before-change results](block-groups-before-comparison.json) retain the comparisons.

The command recipes cover overlapping source groups, root groups with ordinary
peers, grouped repeated nested references, sequential root explosions and a
two-root command with and without output grouping. An unrelated peer does not
become grouped with the exploded pieces. Unused original groups remain in the
table. Block creation, cloned groups and catalog changes share one transaction;
Undo restores original source memberships and removes newly created groups.
Expansion Undo restores root memberships and the original geometry snapshots.

The typed [workflow interface](block-workflow-oracle.md) adds `group`,
`explode_batch` and opt-in group records. Native group indices become local
creation-order indices, with ordered memberships retained. Rhino's hidden
`$block-instance-original-object-id$` marker is excluded only when its value
identifies an object owned by the probe; arbitrary caller text stays visible.
The raw pre-normalization capture and its helper are retained separately.

```sh
tools/rhino_oracle/run_headless.sh rhino \
  tools/rhino_oracle/fixtures/block_groups_commands.json \
  --scheme VibocerosOracleBlockGroupCommandsFinal20261008 --timeout 300 \
  --output /tmp/block-groups.json
python3 -m tools.rhino_oracle replay \
  tools/rhino_oracle/fixtures/block_groups_commands.json \
  --observations tools/rhino_oracle/observations/block_groups_commands.json
```

Native history/selection, protected objects/layers, option memory, linked blocks,
arbitrary imported multi-membership definitions and performance remain outside
this capture. The record schema's existing source-type normalization limitations
also remain explicit in the workflow guide.
