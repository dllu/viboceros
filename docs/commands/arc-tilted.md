# Arc Tilted

Start `Arc`, pick its center, then choose `Tilted`. Pick the start and an
orientation point. Their directions from the center define the arc plane;
the orientation point's component along the radius does not affect that plane.
The radius is the spatial center-to-start distance. Reference geometry or
another viewport can supply the orientation point.

The orientation rectangle and a quarter-arc preview show the proposed plane.
A positive number during orientation fixes the rectangle's height, then a
point chooses its direction. The height affects the guide, leaving the arc's
radius unchanged. Collinear directions reprompt without changing the model.

At the final prompt, pick an endpoint or enter a signed angle in degrees.
Dragging across the start ray changes sweep direction; moving across the
opposite side preserves the chosen sweep. The initial cursor location also
matters. A viewport preview and final click use the same input state.

Choose `Length` to enter a signed arc length, with document units, or pick
two distance points. Their spatial distance supplies the length.
`Direction=Clockwise` and `Direction=Counterclockwise` choose the length's
direction. Length mode starts counterclockwise for each new construction;
negative length reverses it. Angles and lengths clamp to one revolution.

```text
Arc Center 0,0,0 Tilted 4,0,0 0,3,3 90
Arc Center 0,0,0 Tilted 4,0,0 Height=15 0,1,3 -90
Arc Center 0,0,0 Tilted 4,0,0 0,3,3 Length Direction=Clockwise 5
Arc Center 0,0,0 Tilted 4,0,0 0,3,3 Length 7,8,9 10,12,9
```

The command API also accepts an endpoint direction point, projected into the
fitted plane and scaled to the radius. `Direction=Clockwise|Counterclockwise`
before that point fixes its sweep. For example, an interactive partial prefix
`A Center w2,3,4 Tilted` establishes the center as the relative input base.
The common point pipeline supplies coordinates, relative points, filters,
constraints and snaps. Orientation and length measurements can use different
viewports; endpoint picking uses the fitted plane. The viewport CPlane remains
the coordinate reference for typed input.

There are no temporary model objects. Completion adds one Arc history entry;
Esc or another command discards the in-progress construction. Tangent's saved
radius is independent. Rejected points or numbers leave the construction state
available for correction.

[Validation](../arc-tilted-validation.json) records native getter receipts,
full NURBS comparisons, input calibration and remaining limits. Typed endpoint
tokens did not complete Rhino's tilted getter in the diagnostic captures;
endpoint parity uses physical mouse input. Zero-radius and zero-sweep native
failures are retained outside the successful workflow profiles.

Reference: McNeel's public [Arc command documentation](https://docs.mcneel.com/rhino/8/help/en-us/commands/arc.htm).
