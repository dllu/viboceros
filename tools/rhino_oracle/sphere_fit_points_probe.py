"""Owned Sphere FitPoints commands and public SDK fits on original points."""
import json
import os
if globals().get('__package__'):
    from .line_through_points_probe import validate as validate_line
    from .plane_through_points_probe import run_owned
else:
    from line_through_points_probe import validate as validate_line
    from plane_through_points_probe import run_owned


def request():
    with open(os.path.join(os.path.dirname(__file__),'fixtures','sphere_fit_points.json'))as stream:
        return json.load(stream)


def validate(op):
    if not isinstance(op,dict)or op.get('op')!='sphere_fit_points':raise ValueError('invalid sphere fit recipe')
    validate_line(dict(op,op='line_through_points'))


def sphere_value(geo,host):
    G=host['Rhino'].Geometry
    surface=geo.Surfaces[0]if isinstance(geo,G.Brep)else geo
    ok,sphere=surface.TryGetSphere()
    if not ok:raise ValueError('fitted output is not a sphere')
    return dict(center=host['_xyz'](sphere.Center),radius=float(sphere.Radius),definition=host['_nurbs_surface_definition'](surface))


def run(op,host,iterations,tolerance):
    validate(op)
    result,elapsed=run_owned(op,host,iterations,tolerance,'Sphere _FitPoints',sphere_value,' _Enter _Cancel')
    sphere=host['Rhino'].Geometry.Sphere.FitSphereToPoints([host['_point'](p)for p in op['points']])
    result['sdk']=dict(created=bool(sphere.IsValid),center=host['_xyz'](sphere.Center)if sphere.IsValid else None,
                       radius=float(sphere.Radius)if sphere.IsValid else None)
    return result,elapsed
