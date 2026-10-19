"""Actual command geometry, including a completed Cancel with unchanged input."""
KINDS = ('curve_insert_control_point_geometry', 'surface_insert_control_point_geometry')


def command_geometry(event_source, run, snapshot):
    from join_probe import observe_command
    _, value, events = observe_command(event_source, 'InsertControlPoint',
        run, snapshot, lambda: [], True)
    results = [e['result'] for e in events if e['name'] == 'InsertControlPoint']
    if results not in (['Success'], ['Cancel']):
        raise ValueError('insertion command failed: ' + str(results))
    return value


def validate(op):
    if not isinstance(op, dict) or op.get('op') not in KINDS:
        raise ValueError('invalid insertion geometry operation')
    if type(op.get('midpoint', False)) is not bool:
        raise ValueError('insertion midpoint must be boolean')
    if 'parameter' not in op:
        raise ValueError('insertion requires a parameter')
    if op['op'] == KINDS[0]:
        if not isinstance(op.get('curve'), dict):
            raise ValueError('insertion requires a curve definition')
    elif op.get('direction') not in ('u', 'v') or not isinstance(op['parameter'], list) or len(op['parameter']) != 2:
        raise ValueError('surface insertion requires one axis and two parameters')


def run(op, host, iterations, tolerance):
    validate(op)
    R = host['Rhino']; doc = R.RhinoDoc.ActiveDoc; S = host['System']
    surface = op['op'] == KINDS[1]
    source = host['_nurbs_surface_from_definition'](op) if surface else host['_nurbs_curve_from_definition'](op['curve'])
    try:
        raw = op['parameter'] if surface else [op['parameter']]
        parameters = [host['_finite'](t, 'control insertion parameter') for t in raw]
        domains = [source.Domain(i) for i in range(2)] if surface else [source.Domain]
        if any(not d.T0 <= t <= d.T1 for d, t in zip(domains, parameters)):
            raise ValueError('control insertion parameter is outside the active domain')
        point = source.PointAt(*parameters)
        direction = ('_Direction=_' + ('V' if op['direction'] == 'u' else 'U') + ' ') if surface else ''
        command = '_InsertControlPoint ' + direction + '_Midpoint=_' + ('Yes' if op.get('midpoint') else 'No') + ' w' + host['_command_point'](host['_xyz'](point)) + ' _Enter'

        def insert():
            key = S.Guid.Empty
            try:
                doc.Objects.UnselectAll()
                if surface:
                    R.RhinoApp.RunScript('_-CreaseSplitting _Disable', False)
                key = doc.Objects.AddSurface(source) if surface else doc.Objects.AddCurve(source)
                if key == S.Guid.Empty:
                    raise ValueError('could not add insertion source')
                doc.Objects.Select(key, True)

                def snapshot():
                    obj = doc.Objects.FindId(key)
                    if obj is None:
                        raise ValueError('insertion removed the source')
                    geometry = obj.Geometry
                    if surface:
                        if isinstance(geometry, R.Geometry.Brep):
                            if geometry.Faces.Count != 1:
                                raise ValueError('insertion returned a polysurface')
                            geometry = geometry.Faces[0].UnderlyingSurface()
                        result = geometry.ToNurbsSurface()
                    else:
                        result = geometry.ToNurbsCurve()
                    if result is None:
                        raise ValueError('insertion returned no NURBS geometry')
                    try:
                        if surface:
                            value = host['_nurbs_surface_definition'](result)
                            value.update(periodic_u=bool(result.IsPeriodic(0)), periodic_v=bool(result.IsPeriodic(1)))
                        else:
                            value = host['_nurbs_curve_definition'](result)
                            value.update(closed=bool(result.IsClosed), periodic=bool(result.IsPeriodic))
                        return value
                    finally:
                        result.Dispose()

                return command_geometry(R.Commands.Command,
                    lambda: R.RhinoApp.RunScript(command, False), snapshot)
            finally:
                if key != S.Guid.Empty and doc.Objects.FindId(key) is not None:
                    doc.Objects.Delete(key, True)
                if surface:
                    R.RhinoApp.RunScript('_-CreaseSplitting _Enable', False)

        return host['_measure'](iterations, insert)
    finally:
        source.Dispose()
