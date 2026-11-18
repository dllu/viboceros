# Polyline

`Polyline` builds one curve from a sequence of line and circular arc segments.
After the first point, use `Mode=Line` or `Mode=Arc` to choose the next segment.
Bare `Mode` switches between them. Arc mode follows the preceding end tangent.
`Direction` sets a tangent, `ThroughPoint` adds an intermediate point, and
`Center` takes a center and endpoint. After an earlier segment, the endpoint
can also be set by an angle in degrees. The candidate line or arc updates as
you move the cursor.

In Line mode, type a length and then pick its direction. After one segment,
`Length` constrains the next endpoint along the previous end tangent. In Arc
mode, type a radius before picking the endpoint. `Helpers=Yes` displays and
captures nearby CPlane axes and the tangent. Press Tab with the cursor over the
viewport to lock its direction and suspend helper tracking for the next pick.
[Rhino Polyline](https://docs.mcneel.com/rhino/8/help/en-us/commands/polyline.htm)

`Undo` removes a segment from the draft. Press Enter to finish, or use `Close`
after at least two segments. `PersistentClose=Yes` previews the closing segment
throughout drawing and applies it when you finish. The completed curve is one
Undo step. Mode, Helpers, and PersistentClose carry over between interactive runs.

```text
Polyline 0,0,0 4,0,0 4,3,0
Polyline 0 Mode=Line w4,0,0 Mode=Arc w7,3,0 Enter
Polyline 0 Mode=Line w4,0,0 Mode=Arc ThroughPoint w6,1,2 w7,3,0 Enter
```
