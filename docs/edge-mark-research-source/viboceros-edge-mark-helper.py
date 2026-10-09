def run(operation, tolerance, host):
    import Rhino, System
    doc = Rhino.RhinoDoc.ActiveDoc
    ids = []
    outputs = []
    settings = Rhino.DocObjects.ObjectEnumeratorSettings()
    try:
        for source in operation['sources']:
            geometry = host['_object_source'](source, tolerance)
            try:
                key = doc.Objects.Add(geometry)
                if key == System.Guid.Empty:
                    raise ValueError('source admission failed')
                ids.append(key)
            finally:
                geometry.Dispose()
        doc.Objects.UnselectAll()
        for key in ids:
            doc.Objects.Select(key)
        before = set(obj.Id for obj in doc.Objects.GetObjectList(settings))
        command = 'ZoomNonManifold' if operation['id'].startswith('non-manifold') else 'ZoomNaked'
        suffix = operation['id'].rsplit('|', 1)[-1]
        focus = {'current':'', 'all':'_All ', 'next1':'_Next ', 'next2':'_Next _Next ', 'next3':'_Next _Next _Next ', 'next4':'_Next _Next _Next _Next ', 'previous1':'_Previous ', 'next-current':'_Next _Current ', 'all-current':'_All _Current '}.get(suffix, '')
        macro = '_' + command + ' ' + focus + '_Mark _Enter'
        succeeded = Rhino.RhinoApp.RunScript(macro, False)
        objects = [obj for obj in doc.Objects.GetObjectList(settings) if obj.Id not in before and not obj.IsDeleted]
        outputs = [obj.Id for obj in objects]
        points = [[float(obj.Geometry.Location.X), float(obj.Geometry.Location.Y), float(obj.Geometry.Location.Z)] for obj in objects if isinstance(obj.Geometry, Rhino.Geometry.Point)]
        return dict(succeeded=bool(succeeded), points=points,
                    output_count=len(objects), input_count=sum(doc.Objects.FindId(key) is not None for key in ids),
                    prompt=str(Rhino.RhinoApp.CommandPrompt), serials=[int(obj.RuntimeSerialNumber) for obj in objects]), 0
    finally:
        Rhino.RhinoApp.RunScript('_ShowEdgesOff', False)
        for key in outputs + ids:
            doc.Objects.Delete(key, True)
