"""Public auto-sorted position network construction, on privately owned curves."""
import json
import os
if globals().get('__package__'):
    from .curve_recipe import finite,vector,validate_curve
else:
    from curve_recipe import finite,vector,validate_curve
try:
    string_types=(basestring,)
except NameError:
    string_types=(str,)

def request():
    with open(os.path.join(os.path.dirname(__file__),'fixtures','network_surface.json')) as stream:return json.load(stream)

def validate(op):
    required={'op','id','curves','queries','edge_tolerance','interior_tolerance'}
    if (not isinstance(op,dict) or not required<=set(op) or set(op)-required-{'source_sample_fractions'}
            or op['op']!='network_surface' or not isinstance(op['id'],string_types)
            or not isinstance(op['curves'],list) or not 4<=len(op['curves'])<=64
            or not isinstance(op['queries'],list) or not 1<=len(op['queries'])<=64
            or not all(vector(q) for q in op['queries'])
            or any(not finite(op[k]) or op[k]<=0 for k in ['edge_tolerance','interior_tolerance'])):
        raise ValueError('invalid network surface fields')
    if sum(validate_curve(c) for c in op['curves'])>2048:raise ValueError('network control limit')
    if 'source_sample_fractions' in op:
        samples=op['source_sample_fractions']
        if (not isinstance(samples,list) or len(samples)>65
                or any(not finite(s) or not 0<=s<=1 for s in samples)
                or any(a>=b for a,b in zip(samples,samples[1:]))):
            raise ValueError('invalid network source sample fractions')

def run(op,host,iterations,tolerance):
    validate(op);G=host['Rhino'].Geometry;sources=[]
    try:
        for definition in op['curves']:
            source=host['_nurbs_curve_from_definition'](definition)
            if source is None:raise ValueError('SDK network curve construction failed')
            sources.append(source)
        queries=[host['_point'](q) for q in op['queries']]
        def compute():
            copies=[];surface=None
            try:
                for source in sources:
                    c=source.DuplicateCurve()
                    if c is None or c is source:raise ValueError('SDK network private copy failed')
                    copies.append(c)
                surface,error=G.NurbsSurface.CreateNetworkSurface(copies,1,op['edge_tolerance'],op['interior_tolerance'],tolerance['angular'])
                if surface is None:return dict(created=False,error=int(error),valid=False,area=None,closest_points=[])
                if error!=0:raise ValueError('SDK network error with non-null surface')
                points=[]
                for q in queries:
                    ok,u,v=surface.ClosestPoint(q)
                    if not ok:raise ValueError('SDK network closest point failed')
                    points.append(host['_xyz'](surface.PointAt(u,v)))
                # Rhino's area integrator can fail at a remote world origin.
                # Measure an owned translated copy; keep all shape queries and
                # the retained exact definition in the original world frame.
                area_copy=surface.Duplicate()
                if area_copy is None or area_copy is surface:raise ValueError('SDK network area copy failed')
                try:
                    origin=surface.PointAt(surface.Domain(0).T0,surface.Domain(1).T0)
                    if not area_copy.Transform(G.Transform.Translation(-origin.X,-origin.Y,-origin.Z)):
                        raise ValueError('SDK network area translation failed')
                    mass=G.AreaMassProperties.Compute(area_copy)
                    if mass is None:raise ValueError('SDK network area failed')
                    try:
                        area=float(mass.Area)
                        if not finite(area) or area<=0:raise ValueError('invalid SDK network area')
                    finally:mass.Dispose()
                finally:area_copy.Dispose()
                result=dict(created=True,error=0,valid=bool(surface.IsValid),area=area,closest_points=points,
                            definition=host['_nurbs_surface_definition'](surface))
                if op.get('source_sample_fractions'):
                    samples=[]
                    for index,source in enumerate(sources):
                        for fraction in op['source_sample_fractions']:
                            parameter=source.Domain.ParameterAt(fraction)
                            target=source.PointAt(parameter)
                            ok,u,v=surface.ClosestPoint(target)
                            if not ok:raise ValueError('SDK network source closest point failed')
                            u=host['_checked_closest_parameter'](surface.Domain(0),u)
                            v=host['_checked_closest_parameter'](surface.Domain(1),v)
                            closest=surface.PointAt(u,v)
                            distance=closest.DistanceTo(target)
                            if not target.IsValid or not closest.IsValid or not finite(distance) or distance<0:
                                raise ValueError('invalid SDK network source distance')
                            samples.append(dict(source_curve_index=index,source_fraction=fraction,
                                source_parameter=parameter,source_point=host['_xyz'](target),
                                parameters=[u,v],normalized_parameters=[surface.Domain(0).NormalizedParameterAt(u),surface.Domain(1).NormalizedParameterAt(v)],
                                point=host['_xyz'](closest),distance=distance))
                    result['source_samples']=samples
                return result
            finally:
                if surface is not None:surface.Dispose()
                for c in copies:c.Dispose()
        return host['_measure'](iterations,compute)
    finally:
        for c in sources:c.Dispose()
