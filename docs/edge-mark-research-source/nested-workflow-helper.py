"""Observe edge topology through public RhinoCommon APIs, without UI mutation."""


def validate(operation):
    if not {'op', 'id', 'sources'}.issubset(operation) or set(operation) - {'op', 'id', 'sources', 'workflow'}:
        raise ValueError('invalid edge analysis fields')
    sources = operation['sources']
    if not isinstance(sources, list) or not 1 <= len(sources) <= 64:
        raise ValueError('edge analysis requires 1..64 sources')
    if any(not isinstance(source, dict) or source.get('type') not in
           ('surface', 'brep', 'box_brep', 'mesh') for source in sources):
        raise ValueError('edge analysis requires surfaces, Breps or meshes')
    if 'workflow' in operation:
        w = operation['workflow']
        if (not isinstance(w, dict) or set(w) - {'command', 'actions', 'undo_redo'}
                or w.get('command') not in ('ZoomNaked', 'ZoomNonManifold')
                or not isinstance(w.get('actions'), list) or len(w['actions']) > 32
                or any(n not in ('All', 'Current', 'Next', 'Previous', 'Mark') for n in w['actions'])
                or type(w.get('undo_redo', False)) is not bool):
            raise ValueError('invalid edge analysis workflow')


def run(operation, tolerance, host):
    validate(operation)
    if 'workflow' in operation:
        return run_workflow(operation, tolerance, host)
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


def run_workflow(operation, tolerance, host):
    import Rhino, System
    doc = Rhino.RhinoDoc.ActiveDoc
    ids = []
    outputs = []
    settings = Rhino.DocObjects.ObjectEnumeratorSettings()

    def snapshot():
        objects = sorted((o for o in doc.Objects.GetObjectList(settings)
                          if o.Id not in ids and isinstance(o.Geometry, Rhino.Geometry.Point)),
                         key=lambda o: int(o.RuntimeSerialNumber))
        return dict(points=[[float(o.Geometry.Location.X), float(o.Geometry.Location.Y), float(o.Geometry.Location.Z)] for o in objects],
                    input_count=sum(doc.Objects.FindId(key) is not None and not doc.Objects.FindId(key).IsDeleted for key in ids))

    try:
        serial = doc.BeginUndoRecord('Edge analysis owned sources')
        try:
            for source in operation['sources']:
                geometry = host['_object_source'](source, tolerance)
                try:
                    key = doc.Objects.Add(geometry)
                    if key == System.Guid.Empty:
                        raise ValueError('edge analysis source admission failed')
                    ids.append(key)
                finally:
                    geometry.Dispose()
        finally:
            doc.EndUndoRecord(serial)
        doc.Objects.UnselectAll()
        for key in ids:
            doc.Objects.Select(key)
        workflow = operation['workflow']
        command = workflow['command'] + ' ' + ' '.join(workflow['actions']) 
        macro = '_' + workflow['command'] + ' ' + ' '.join('_' + n for n in workflow['actions']) + ' _Enter'
        succeeded = Rhino.RhinoApp.RunScript(macro, False)
        result = snapshot()
        result.update(succeeded=bool(succeeded), command=command)
        outputs.extend(o.Id for o in doc.Objects.GetObjectList(settings) if o.Id not in ids)
        if workflow.get('undo_redo', False):
            ok = Rhino.RhinoApp.RunScript('_Undo', False)
            result['undo'] = snapshot();result['undo']['succeeded'] = bool(ok)
            ok = Rhino.RhinoApp.RunScript('_Redo', False)
            result['redo'] = snapshot();result['redo']['succeeded'] = bool(ok)
        return result, 0
    finally:
        Rhino.RhinoApp.RunScript('_ShowEdgesOff', False)
        for key in outputs + ids:
            doc.Objects.Delete(key, True)
