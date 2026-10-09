"""Observe edge topology through public RhinoCommon APIs, without UI mutation."""


def validate(operation):
    if set(operation) != {'op', 'id', 'sources'}:
        raise ValueError('invalid edge analysis fields')
    sources = operation['sources']
    if not isinstance(sources, list) or not 1 <= len(sources) <= 64:
        raise ValueError('edge analysis requires 1..64 sources')
    if any(not isinstance(source, dict) or source.get('type') not in
           ('surface', 'brep', 'box_brep', 'mesh') for source in sources):
        raise ValueError('edge analysis requires surfaces, Breps or meshes')


def run(operation, tolerance, host):
    validate(operation)
    import Rhino

    def record(index, curve, all_edges, naked, non_manifold):
        domain = curve.Domain
        points = []
        for i in range(17):
            t = domain.T0 if i == 0 else domain.T1 if i == 16 else domain.T0 + (domain.T1 - domain.T0) * i / 16.0
            p = curve.PointAt(t)
            points.append([float(p.X), float(p.Y), float(p.Z)])
        return dict(index=index, domain=[float(domain.T0), float(domain.T1)],
                    points=points, all=bool(all_edges), naked=bool(naked),
                    non_manifold=bool(non_manifold))

    sources = []
    for source in operation['sources']:
        geometry = host['_object_source'](source, tolerance)
        brep = None
        try:
            edges = []
            if isinstance(geometry, Rhino.Geometry.Mesh):
                topology = geometry.TopologyEdges
                for i in range(topology.Count):
                    count = len(topology.GetConnectedFaces(i))
                    curve = topology.EdgeLine(i).ToNurbsCurve()
                    try:
                        edges.append(record(i, curve, count == 1 or topology.IsEdgeUnwelded(i),
                                            count == 1, count > 2))
                    finally:
                        curve.Dispose()
            else:
                if isinstance(geometry, Rhino.Geometry.Brep):
                    brep = geometry
                else:
                    brep = geometry.ToBrep()
                for edge in brep.Edges:
                    edges.append(record(int(edge.EdgeIndex), edge, True,
                                        edge.Valence == Rhino.Geometry.EdgeAdjacency.Naked,
                                        edge.Valence == Rhino.Geometry.EdgeAdjacency.NonManifold))
            sources.append(dict(edges=edges))
        finally:
            if brep is not None and brep is not geometry:
                brep.Dispose()
            geometry.Dispose()
    return dict(sources=sources), 0
