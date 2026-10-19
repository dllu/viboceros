"""Scaled public curve inputs around existing edit-point and seam decisions."""
try:
    from . import insert_edit_point_probe as base
except (ImportError, ValueError):
    import insert_edit_point_probe as base


def mapped(op):
    result=dict(op,op='insert_edit_point_command');result.pop('scale',None);return result


def validate(op):
    if not isinstance(op,dict) or op.get('op')!='insert_edit_point_limits':raise ValueError('invalid edit-point limit operation')
    scale=op.get('scale',1.)
    if type(scale)not in(int,float)or not 1e-6<=scale<=1e6:raise ValueError('invalid edit-point source scale')
    if scale!=1. and op.get('family')not in('cubic','rational'):raise ValueError('scaled limit source must be a Bezier curve')
    base.validate(mapped(op))


def run(op,host,iterations,tolerance):
    validate(op)
    build=host['_nurbs_curve_from_definition']; projections=[]
    def source(definition):
        d=dict(definition);d['control_points']=[dict(c,point=[x*op.get('scale',1.)for x in c['point']])for c in definition['control_points']]
        n=build(d)
        if op['family']=='cubic':n.Points.MakeNonRational()
        for f in op['stations']:
            t=n.Domain.ParameterAt(f);p=n.PointAt(t);ok,closest=n.ClosestPoint(p)
            projections.append(dict(input_parameter=t,closest_parameter=float(closest),success=bool(ok),point=host['_xyz'](p)))
        return n
    value,elapsed=base.run(mapped(op),dict(host,_nurbs_curve_from_definition=source),iterations,tolerance)
    value['recipe']=op;value['sdk_projections']=projections
    return value,elapsed
