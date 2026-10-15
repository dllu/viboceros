"""Owned Rhino 8 SDK crease splitting; native document/getter capture is separate."""
import json
import math
from pathlib import Path
try:
    from .geometry_validity_probe import validate as validate_source
except ImportError:
    from geometry_validity_probe import validate as validate_source


def request():
    return json.loads(Path(__file__).with_name('fixtures').joinpath('divide_along_creases.json').read_text())


def validate(op):
    if (not isinstance(op,dict) or set(op)!={'op','id','surface','split_at_kinks','split_at_tangents','as_brep'}
            or op['op']!='divide_along_creases' or not isinstance(op['id'],str)
            or any(type(op[k]) is not bool for k in ('split_at_kinks','split_at_tangents','as_brep'))
            or not isinstance(op['surface'],dict)):
        raise ValueError('invalid crease division operation')
    s=op['surface']
    fields={'degree_u','degree_v','control_point_count_u','control_point_count_v','control_points','knots_u','knots_v'}
    if not fields<=set(s) or set(s)-fields-{'domain_u','domain_v'}:
        raise ValueError('invalid crease surface fields')
    validate_source(dict(op='geometry_validity',id=op['id'],source=dict(s,type='surface')))
    for d in ('u','v'):
        p,n=s['degree_'+d],s['control_point_count_'+d];knots=s['knots_'+d]
        if (type(p) is not int or type(n) is not int or not 1<=p<=64 or not p<n<=100000
                or not isinstance(knots,list) or len(knots)!=n+p+1
                or any(a>b for a,b in zip(knots,knots[1:])) or not knots[p]<knots[n]):
            raise ValueError('invalid crease surface basis')
    count=s['control_point_count_u']*s['control_point_count_v'];controls=s['control_points']
    if (count>100000 or not isinstance(controls,list) or len(controls)!=count
            or any(not isinstance(c,dict) or set(c)!={'point','weight'}
                   or not isinstance(c['point'],list) or len(c['point'])!=3
                   or type(c['weight']) not in (int,float) or c['weight']<=0 for c in controls)):
        raise ValueError('invalid crease control net')
    for d in ('u','v'):
        domain=s.get('domain_'+d)
        if (domain is not None and (not isinstance(domain,list) or len(domain)!=2 or not domain[0]<domain[1])):
            raise ValueError('invalid crease surface domain')
    if (s.get('domain_u') is None)!=(s.get('domain_v') is None):
        raise ValueError('both crease surface domains are required')


def run(op,host,iterations,tolerance):
    validate(op)
    surface=host['_nurbs_surface_from_definition'](op['surface'])
    try:
        if surface is None: raise ValueError('SDK surface construction failed')
        source=surface.ToBrep()
        try:
            if source is None or source is surface: raise ValueError('SDK B-rep construction failed')
            if source.Faces.Count!=1: raise ValueError('SDK crease input must have one unsplit face')
            def compute():
                output=source.DuplicateBrep()
                if output is None or output is source: raise ValueError('SDK B-rep copy failed')
                try:
                    if op['split_at_kinks']: output.Faces.SplitKinkyFaces(tolerance['angular'],True)
                    if op['split_at_tangents']: output.Faces.SplitFacesAtTangents()
                    faces=int(output.Faces.Count)
                    if not output.IsValid or output.IsSolid or faces<1: raise ValueError('SDK crease output is invalid')
                    mass=host['Rhino'].Geometry.AreaMassProperties.Compute(output)
                    if mass is None: raise ValueError('SDK crease area failed')
                    try: area=float(mass.Area)
                    finally: mass.Dispose()
                    if not math.isfinite(area) or area<=0: raise ValueError('invalid SDK crease area')
                    adjacency=[len(edge.AdjacentFaces()) for edge in output.Edges]
                    return dict(face_count=faces,added_faces=faces-1,valid=True,is_solid=False,area=area,
                                loop_counts=sorted(int(face.Loops.Count) for face in output.Faces),
                                naked_edges=adjacency.count(1),shared_edges=adjacency.count(2))
                finally: output.Dispose()
            return host['_measure'](iterations,compute)
        finally:
            if source is not None and source is not surface: source.Dispose()
    finally:
        if surface is not None: surface.Dispose()
