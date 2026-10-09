# CreateUniqueBlock and definition duplication

`CreateUniqueBlock new-name` copies the definition used by the selected block
roots and rebinds those roots to the new definition. All selected roots must use
one original definition. They share one new copy; unselected instances remain
on the original. The operation retains root IDs, placement, attributes, geometry
text and model groups. Copied prototype members lose their group memberships,
following the native command. Nested child definitions remain shared.

Bare `CreateUniqueBlock` supports preselection or an instance selection prompt,
then asks for the new name. Quote names containing spaces. Cancel leaves the
document and redo branch unchanged. Name collisions, mixed definitions and
uneditable roots reject before catalog changes. Explicit selected roots are
rebound in one Undo step; Undo restores their original geometry snapshots and
removes the new definition.

```text
Point 1,2,3
SelAll
Block 0,0,0 Part
Insert Part 10,0,0
CreateUniqueBlock "Unique part"
Undo
```

The [BlockManager](block-manager.md) pane also offers **Duplicate definition**
with an editable target name. This creates an unused definition and leaves all
existing instances unchanged. Duplicate definitions share immutable geometry
storage with their source and keep nested references, while clearing prototype
groups. Later definition edits remain independent; edits to a shared nested
child still affect both parent definitions.

Document APIs: `duplicate_block_definition` and `make_block_instances_unique`.
The [workflow protocol](../block-workflow-oracle.md) adds `duplicate_definition`
and `make_unique`, with unique selections validated before native host access.
The native adapter uses the scriptable CreateUniqueBlock command. For an unused
definition copy it creates a temporary insert, makes that insert unique, and
removes it; that verifies the same duplication boundary, rather than the panel
button itself. Logical object handles never change during rebinding.

Five private-Xvfb Rhino 8.32.26160.13001 workflows cover one/multiple roots,
unchanged placement, original peers, shared nested children, subsequent child
redefinition, prototype/root groups and unused copies. The source and raw records
are retained in [provenance](../block-unique-provenance.json).
Exact native name defaults, protected-root/group picking, linked definitions,
panel duplication interaction and performance remain unverified. The local
command asks for an explicit target name rather than assuming a native default.

Reference: [CreateUniqueBlock](https://docs.mcneel.com/rhino/8/help/en-us/commands/block.htm#CreateUniqueBlock).
