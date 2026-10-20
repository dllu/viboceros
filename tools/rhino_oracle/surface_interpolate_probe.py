"""Fixed public surface interpolation observations; no document edits."""
import re
from copy import deepcopy
FIVE=[[.1,.2],[.3,.7],[.5,.35],[.7,.8],[.9,.3]]
CASES={
    'plane_two':('plane',[[.1,.2],[.9,.8]],False),
    'plane_three':('plane',[[.1,.2],[.4,.75],[.9,.3]],False),
    'plane_five':('plane',FIVE,False),
    'plane_closed':('plane',[[.2,.2],[.8,.25],[.7,.8],[.3,.7]],True),
    'skew_five':('skew_plane',FIVE,False),
    'cylinder_axis':('cylinder',[[.15,.1],[.15,.35],[.15,.6],[.15,.9]],False),
    'cylinder_five':('cylinder',[[.1,.2],[.2,.7],[.35,.35],[.45,.8],[.6,.3]],False),
    'cylinder_seam':('cylinder',[[.9,.2],[.96,.7],[.04,.4],[.12,.8]],False),
    'sphere_five':('sphere',[[.1,.35],[.2,.55],[.3,.45],[.4,.75],[.45,.6]],False),
    'sphere_pole':('sphere',[[.1,.5],[.15,.7],[.2,1.]],False),
    'general_five':('general',FIVE,False),
    'rational_five':('rational',FIVE,False),
    'boundary':('general',[[.1,.1],[.4,.95],[.55,.05],[.9,.9]],False),
    'chart':('chart',FIVE,False),
    'coincident':('plane',[[.2,.3],[.2,.3]],False),
}
EXTRA_CASES={
    'cylinder_closed':('cylinder',[[.1,.2],[.4,.3],[.65,.7],[.9,.6]],True),
    'cylinder_closed_reverse':('cylinder',[[.9,.6],[.65,.7],[.4,.3],[.1,.2]],True),
    'cylinder_closed_two':('cylinder',[[.1,.2],[.4,.8]],True),
    'cylinder_closed_local':('cylinder',[[.15,.2],[.35,.7],[.2,.8]],True),
    'cylinder_two_laps':('cylinder',[[.1,.5],[.35,.5],[.6,.5],[.85,.5],[.1,.5],[.35,.5],[.6,.5],[.85,.5]],True),
    'sphere_closed':('sphere',[[.1,.35],[.35,.4],[.6,.7],[.9,.6]],True),
    'plane_overshoot':('plane',[[.1,.1],[.5,1.],[.9,.2]],False),
    'general_overshoot':('general',[[.1,.1],[.5,1.],[.9,.2]],False),
}
SCENES=dict(CASES,**EXTRA_CASES)
def closed_request():
    q=request();q['operations']=[dict(op='surface_interpolate',id=k,case=k,points=deepcopy(v[1]),closed=v[2])for k,v in EXTRA_CASES.items()];return q
def request():
    return deepcopy(dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=[dict(op='surface_interpolate',id=k,case=k,points=v[1],closed=v[2])for k,v in CASES.items()]))
def validate(op):
    if not isinstance(op,dict)or set(op)!={'op','id','case','points','closed'}or op['op']!='surface_interpolate'or not isinstance(op['case'],str)or op['case']not in SCENES:raise ValueError('fixed surface interpolation case required')
    if not isinstance(op['id'],str)or re.match(r'\A[A-Za-z0-9_-]{1,80}\Z',op['id'])is None or op['points']!=SCENES[op['case']][1]or type(op['closed'])is not bool or op['closed']!=SCENES[op['case']][2]:raise ValueError('fixed surface interpolation inputs required')
def validate_request(q):
    if type(q.get('iterations'))is not int or q['iterations']!=1 or not isinstance(q.get('operations'),list)or not 1<=len(q['operations'])<=16:raise ValueError('bounded one-iteration interpolation request required')
    for op in q['operations']:validate(op)
    if len({op['id']for op in q['operations']})!=len(q['operations']):raise ValueError('unique interpolation ids required')
def run(op,host,iterations,tolerance):
    validate(op)
    if iterations!=1:raise ValueError('one interpolation observation required')
    from surface_short_path_probe import source
    G=host['Rhino'].Geometry;surface=source(SCENES[op['case']][0],host)
    curves=[]
    try:
        before=int(surface.DataCRC(0));parameters=[G.Point2d(surface.Domain(0).ParameterAt(p[0]),surface.Domain(1).ParameterAt(p[1]))for p in op['points']]
        points=[surface.PointAt(p.X,p.Y)for p in parameters]
        results={}
        for mode in ['uv','world']:
            error=None;curve=None
            try:
                curve=surface.InterpolatedCurveOnSurfaceUV(parameters,float(tolerance['absolute']),op['closed'],0)if mode=='uv'else surface.InterpolatedCurveOnSurface(points+points[:1]if op['closed']else points,float(tolerance['absolute']))
            except host['System'].ArgumentException as exception:error=str(exception)
            if curve is not None:
                curves.append(curve)
                if not curve.IsValid:raise ValueError('invalid native interpolant')
            pullback=surface.Pullback(curve,1e-8)if curve is not None else None
            if pullback is not None:curves.append(pullback)
            results[mode]=dict(curve=host['_nurbs_curve_definition'](curve)if curve is not None else None,pullback=host['_nurbs_curve_definition'](pullback)if pullback is not None else None,error=error)
        if int(surface.DataCRC(0))!=before:raise ValueError('surface interpolation mutated its source')
        return dict(surface=host['_nurbs_surface_definition'](surface),parameters=[[float(p.X),float(p.Y)]for p in parameters],points=[host['_xyz'](p)for p in points],closed=op['closed'],source_unchanged=True,interpolants=results),0
    finally:
        for curve in curves:curve.Dispose()
        surface.Dispose()
