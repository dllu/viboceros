# ExtractPt curve portions

Start `ExtractPt`, enter `SubCrv`, select a standalone curve or B-rep edge, and pick its start
and directed end. You can also start with `ExtractPt SubCrv`. The shared getter
supports length input, direction locking, units, and preview. Repeat `SubCrv`
to collect more portions, then press Enter in the parent getter to extract.
Escape cancels without changing geometry or consuming Redo.

Typed input accepts repeatable ranges in each source curve's parameter domain:

```text
ExtractPt SubCrv=uuid,start,end Output=Points OutputLayer=Input
ExtractPt SubCrv=uuid,Edge,index,start,end Output=PointCloud
ExtractPt SubCrv=uuid,EdgeWrap,index,start,end Output=Points
```

For an edge, parameters use its resolved source representation: imported analytic
arcs keep their native domain and circular parameter speed; NURBS edges retain
their original controls and parameterization. During the getter, Ctrl+Shift-click
an edge, or enter `Edge=uuid,index`. Source geometry and tolerance are checked
again before extraction.

Closed standalone curves can wrap across their seam. A backward edge portion
reverses the interval between the two picks, including on a closed edge.
Numeric edge input can wrap through the seam and retains the numeric getter's
endpoint ordering. `EdgeWrap` represents those intervals in typed input.
Each portion contributes its own
defining points; the source curve and its stored controls remain unchanged.
Whole-object selections contribute first, then temporary portions in pick order,
then selected grips. A whole curve and portions of that curve both contribute.
Component inputs retain their usual order after those inputs.

Temporary portions produce fresh current-layer attributes and no groups, even
with `OutputLayer=Input`. Whole objects and grips keep their ordinary attribute
rules. Input-layer clouds use the first contributing whole input's attributes;
when grips contribute, the first grip owner's attributes take precedence.

Results are selected immediately. Undo/Redo releases their selection and turns
off grip display on the temporary portions' owners. Geometry and output creation
form one Undo step. Changed or unavailable sources and invalid or excessive
ranges reject the operation before model edits. The output-point limit counts
points from all whole objects, portions, grips and components together.

The [qualification](../extract-point-ranges-qualification.json) compares 54 native
scenes through typed commands and 108 shared-getter workflows, at `1e-7`. They
cover lines, rational and periodic NURBS, polylines, polycurves, circular wrapping,
multiple portions, mixed selections, both output types, attributes and history.
Edge scenes cover boxes, trimmed and rational faces, joined periodic surfaces,
and circular edges stored as either analytic arcs or NURBS, including numeric
length confirmation across the seam. The original C3
controls remain unchanged. Version 3 of the internal B-rep payload preserves
analytic arc provenance; versions 1 and 2 remain readable.

Six earlier discovery captures used construction-plane endpoint coordinates or
incomplete numeric endpoint receipts. They are retained as diagnostics and
excluded from this qualification.
