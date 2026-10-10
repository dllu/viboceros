"""Fixed public SDK rolling-ball edge-fillet references; no document writes."""
import math
import re

CASES = ('box-single', 'box-corner', 'box-all', 'box-too-large')


def validate(operation):
    if (set(operation) != {'op', 'id', 'case'}
            or operation.get('op') != 'fillet_edge_reference'
            or not isinstance(operation.get('id'), (str, type(u'')))
            or re.match(r'^[A-Za-z0-9_-]{1,80}\Z', operation['id']) is None
            or not isinstance(operation.get('case'), (str, type(u'')))
            or operation['case'] not in CASES):
        raise ValueError('invalid fixed edge fillet fixture')


def run(operation, host):
    validate(operation)
    Rhino = host['Rhino']
    from System import Array, Int32, Double
    origin = Rhino.Geometry.Point3d(0., 0., 0.)
    source = Rhino.Geometry.Brep.CreateFromBox(Rhino.Geometry.BoundingBox(
        origin, Rhino.Geometry.Point3d(10., 10., 10.)))
    outputs = []
    try:
        case = operation['case']
        if case == 'box-single':
            edges = [e.EdgeIndex for e in source.Edges
                     if e.PointAt(e.Domain.Mid).DistanceTo(Rhino.Geometry.Point3d(0., 0., 5.)) < 1e-8]
        elif case == 'box-corner':
            edges = [e.EdgeIndex for e in source.Edges
                     if min(e.PointAtStart.DistanceTo(origin), e.PointAtEnd.DistanceTo(origin)) < 1e-8]
        else:
            edges = [e.EdgeIndex for e in source.Edges]
        radius = 6. if case == 'box-too-large' else 1.
        radii = Array[Double]([radius]*len(edges))
        outputs = list(Rhino.Geometry.Brep.CreateFilletEdges(source, Array[Int32](edges), radii, radii,
            Rhino.Geometry.BlendType.Fillet, Rhino.Geometry.RailType.RollingBall,
            False, 1e-9, 1e-10) or [])
        records = []
        for brep in outputs:
            properties = Rhino.Geometry.VolumeMassProperties.Compute(brep,
                True, False, False, False, 1e-10, 1e-10)
            try:
                records.append(dict(valid=bool(brep.IsValid), solid=bool(brep.IsSolid),
                    volume=None if properties is None else float(properties.Volume),
                    geometry=host['_interchange_brep_record'](brep)))
            finally:
                if properties is not None:
                    properties.Dispose()
        return dict(selected_edges=edges, radius=radius, outputs=records), 0
    finally:
        for brep in outputs:
            brep.Dispose()
        source.Dispose()
