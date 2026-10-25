# SubD display

`SubDDisplayToggle` switches all SubDs between their smooth limit surface and
flat control net. It works during other commands and preserves object, component
and grip selections. Empty command input or viewport focus also accepts Tab;
nonempty input retains command completion, and point input retains Tab.

Flat display uses the public OpenNURBS control mesh with faceted shading.
Wireframe, shaded and ghosted modes share that geometry, component ordinals and
hit testing. Blocks and transform previews follow the same appearance. This
display setting does not create an Undo step or consume Redo.

`ExtractPt` follows the display with `SubDLocation=Display`, its default.
Choose `Surface` or `ControlNet` to override it for whole objects or components;
selected grips keep their displayed locations. Appearance survives geometry
Undo/Redo. It is currently a session setting and is not saved in 3DM files.

The [qualification](../subd-display-qualification.json) compares 36 native scenes
through commands and nested UI getters, including whole objects, selected control
and edit grips, both appearances, attributes and history. Renderer tests check
native control locations, face maps, faceted normals, n-gon fans and cache reuse.

See the [Rhino reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/subddisplaytoggle.htm).
