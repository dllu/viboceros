"""Public native component identities, retaining their ordinal correspondence."""


def indexes(geometry, kind, host):
    G = host['Rhino'].Geometry
    if isinstance(geometry, G.SubD):
        if kind == 'face':
            return [f.ComponentIndex() for f in geometry.Faces]
        if kind == 'edge':
            return [e.ComponentIndex() for e in geometry.Edges]
        values, vertex = [], geometry.Vertices.First
        while vertex is not None:
            values.append(vertex.ComponentIndex())
            vertex = vertex.Next
        return values
    member, count = {'face':('MeshFace',geometry.Faces.Count),
                     'edge':('MeshTopologyEdge',geometry.TopologyEdges.Count),
                     'vertex':('MeshTopologyVertex',geometry.TopologyVertices.Count)}[kind]
    return [G.ComponentIndex(getattr(G.ComponentIndexType, member), i) for i in range(count)]


def selected(obj, host):
    G = host['Rhino'].Geometry
    if not isinstance(obj.Geometry, (G.Mesh, G.SubD)):
        return []
    groups = {kind:indexes(obj.Geometry,kind,host) for kind in ['face','edge','vertex']}
    values = []
    for component in (obj.GetSelectedSubObjects() or []):
        G = host['Rhino'].Geometry
        if isinstance(obj.Geometry, G.Mesh) and component.ComponentIndexType == G.ComponentIndexType.MeshVertex:
            values.append(['vertex', int(obj.Geometry.TopologyVertices.TopologyVertexIndex(component.Index))])
            continue
        for kind, candidates in groups.items():
            for i, candidate in enumerate(candidates):
                if component == candidate:
                    values.append([kind,i])
    return values


def select(doc, ids, case, host):
    for source, kind, index in case['components']:
        obj = doc.Objects.FindId(ids[source])
        component = indexes(obj.Geometry,kind,host)[index]
        if not obj.SelectSubObject(component, True, True, False):
            raise ValueError('owned ExtractPt component selection failed')
