# Box and MeshBox

Run `Box` or `MeshBox`, pick opposite corners of the base, then pick a
height point or type a signed height. Enter uses the base width. Hold Shift
while picking the opposite corner to make a square base. The viewport
previews the base outline and the height before creating geometry.

- `Center` starts from the center of the base.
- `3Point` uses two points on the first edge and a third point to set the
  width and plane. Choose `EdgeMidpoint` inside `3Point` to start from the
  edge midpoint.
- `Vertical` uses the construction plane’s vertical direction in the base.
- `Diagonal` accepts two opposite corners in 3D. If their elevations match,
  pick a separate height point. Inside `Diagonal`, choose `Cube` to
  project the second corner onto a cube diagonal.
- `AroundCurve` in `MeshBox` uses a selected curve or surface edge to
  position the center and orient the base perpendicular to the guide.

`Box` follows [UseExtrusions](use-extrusions.md). `MeshBox` exposes
`XCount`, `YCount`, and `ZCount` before the base. The interactive getter
starts at ten faces per direction and remembers the latest counts for the
session. A positive integer at the initial MeshBox prompt sets all three
counts. The existing coordinate-only MeshBox syntax defaults to one face
per direction when no counts are supplied.

```text
Box 3Point EdgeMidpoint 1,2,3 4,6,3 0,5,5 4
Box Diagonal Cube 1,2,3 5,8,10
MeshBox XCount=2 YCount=3 ZCount=1 Vertical 1,2,3 4,6,3 0,5,8 4
```

Each completed box uses one undo step. Escape discards the preview.

[Rhino Box](https://docs.mcneel.com/rhino/8/help/en-us/commands/box.htm) and
[Rhino MeshBox](https://docs.mcneel.com/rhino/8/help/en-us/commands/meshbox.htm).
