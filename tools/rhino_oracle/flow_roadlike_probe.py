"""Owned Flow Roadlike commands; CPlane-Z presets become picked directions."""
import math
if globals().get('__package__'):
    from . import flow_line_probe
else:
    import flow_line_probe


def line_recipe(op):
    return dict((k,'flow_line'if k=='op'else v)for k,v in op.items()if k not in('base_up','target_up','cplane_up','native_automatic'))


def validate(op):
    if not isinstance(op,dict)or op.get('op')!='flow_roadlike':raise ValueError('invalid Roadlike recipe')
    if type(op.get('native_automatic',False))is not bool:raise ValueError('invalid Automatic diagnostic')
    flow_line_probe.validate(line_recipe(op),allow_selected_guides=True)
    for name in('base_up','target_up','cplane_up'):
        up=op.get(name)
        if up is None:continue
        if not isinstance(up,list)or len(up)!=3 or any(type(x)not in(int,float)or not -1e6<=x<=1e6 or math.isnan(x)or math.isinf(x)for x in up)or max(abs(x)for x in up)<1e-6:raise ValueError('invalid Roadlike axis')


def run(op,host,iterations,tolerance):
    validate(op)
    R=host['Rhino'];vp=R.RhinoDoc.ActiveDoc.Views.ActiveView.ActiveViewport;saved=vp.GetConstructionPlane().Plane
    try:
        if op.get('cplane_up') is not None:
            vp.SetConstructionPlane(R.Geometry.Plane(R.Geometry.Point3d.Origin,R.Geometry.Vector3d(*op['cplane_up'])))
        preset=host['_xyz'](vp.GetConstructionPlane().Plane.ZAxis)
        axes=[op.get('base_up'),op.get('target_up')]
        if not op.get('native_automatic'):axes=[preset if up is None else up for up in axes]
        return flow_line_probe.run(line_recipe(op),host,iterations,tolerance,axes)
    finally:vp.SetConstructionPlane(saved)
