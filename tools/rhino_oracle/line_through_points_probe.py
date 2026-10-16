"""Owned LineThroughPt commands and public SDK line fits on original points."""
import json
import os
if globals().get('__package__'):
    from .plane_through_points_probe import validate as validate_plane, run_owned
else:
    from plane_through_points_probe import validate as validate_plane, run_owned


def request():
    with open(os.path.join(os.path.dirname(__file__), 'fixtures', 'line_through_points.json')) as stream:
        return json.load(stream)


def validate(op):
    if (not isinstance(op,dict) or set(op)!={'op','id','points','source','postselect'}
            or op['op']!='line_through_points' or type(op['postselect']) is not bool
            or op['source'] not in ('points','cloud','mesh','mesh_grips','curve_grips')):
        raise ValueError('invalid LineThroughPt recipe')
    candidate=dict(op)
    candidate['op']='plane_through_points'
    if op['source'].endswith('_grips') and op['postselect']:
        raise ValueError('grip recipes require preselection')
    if op['source']=='curve_grips' and isinstance(op['points'],list) and len(op['points'])==2:
        candidate['source']='points'
    if candidate.get('source')=='mesh':candidate['source']='mesh_grips';candidate['postselect']=False
    validate_plane(candidate)


def curve_value(geo,host):
    return dict(definition=host['_nurbs_curve_definition'](geo),
                endpoints=[host['_xyz'](geo.PointAtStart),host['_xyz'](geo.PointAtEnd)])


def run(op,host,iterations,tolerance):
    validate(op)
    value,elapsed=run_owned(op,host,iterations,tolerance,'LineThroughPt',curve_value)
    ok,line=host['Rhino'].Geometry.Line.TryFitLineToPoints([host['_point'](p)for p in op['points']])
    value['sdk']=dict(created=bool(ok),endpoints=[host['_xyz'](line.From),host['_xyz'](line.To)]if ok else [])
    return value,elapsed
