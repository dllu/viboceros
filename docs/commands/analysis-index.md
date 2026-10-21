# Measurement and geometry analysis

[All commands](README.md) · [Command input](../command-line.md)

- [Check and SelBadObjects](check.md)
- [ShowEdges, ShowEdgesOff, ZoomNaked and ZoomNonManifold](show-edges.md)
- [Fit a line through points, clouds, meshes and grips](line-through-points.md)
- [PointDeviation distance analysis](point-deviation.md)
- [Extraction, measurement, and intersections](editing.md)
- [Angle, distance, length, area, and volume queries](measurements.md)
- [Cumulative area-centroid markers](area-centroid.md)
- [Signed scalar volume and open-boundary confirmation](volume.md)
- [Signed-volume centroid markers](volume-centroid.md)
- [Local curve radius and diameter](radius.md)
- [Persistent curve and surface curvature graphs](curvature-graph.md)
- [Extract curvature graph outlines into model polylines](extract-curvature-graph.md)
- [Endpoint geometric continuity](gcon.md)
- [World and construction-plane point coordinates](evaluate-point.md)
- [Curve and surface parameter domains](domain.md)
- [Surface UV coordinates of a point](evaluate-uv-point.md)

- [Curve deviation reports, temporary portions, and compatibility limits](crv-deviation.md)

## Examples

These are independent snippets. Prepare the required sources and selection first;
replace object IDs, names and paths with values from your document. See each
command page for supported options and limitations.

```text
PolygonCount
Length
Curvature MarkCurvature=No 2,0,0
Area
AreaCentroid
Volume
VolumeCentroid
CrvStart
CrvEnd
GCon
GCon Pick1=0,0,0 Pick2=4,0,0
```
