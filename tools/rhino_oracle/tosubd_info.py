"""Public surface continuity queries for conversion discovery."""
def surface_info(surface, host):
    G = host['Rhino'].Geometry
    result = dict(closed=[bool(surface.IsClosed(a)) for a in range(2)],
                  periodic=[bool(surface.IsPeriodic(a)) for a in range(2)],
                  singular=[bool(surface.IsSingular(a)) for a in range(4)],
                  discontinuities={})
    for name in ['C0_continuous', 'C1_continuous', 'G1_continuous', 'G1_locus_continuous']:
        axes = []
        for axis in range(2):
            domain = surface.Domain(axis)
            start, end = float(domain.T0), float(domain.T1)
            breaks = []
            for _ in range(1000):
                found, parameter = surface.GetNextDiscontinuity(axis, getattr(G.Continuity, name), start, end)
                if not found:
                    break
                parameter = float(parameter)
                if parameter <= start or parameter > end:
                    raise ValueError('invalid surface discontinuity traversal')
                breaks.append(parameter)
                start = parameter
            else:
                raise ValueError('surface discontinuity traversal limit')
            axes.append(breaks)
        result['discontinuities'][name] = axes
    result['pole_normals'] = []
    for side,singular in enumerate(result['singular']):
        if not singular:
            continue
        normals=[]
        for i in range(8):
            t=i/8.
            du,dv=surface.Domain(0),surface.Domain(1)
            u=du.ParameterAt(t) if side in (0,2) else (du.T0 if side==3 else du.T1)
            v=dv.ParameterAt(t) if side in (1,3) else (dv.T0 if side==0 else dv.T1)
            found,frame=surface.FrameAt(u,v)
            normals.append([float(frame.ZAxis.X),float(frame.ZAxis.Y),float(frame.ZAxis.Z)] if found else None)
        result['pole_normals'].append(dict(side=side,normals=normals))
    return result
