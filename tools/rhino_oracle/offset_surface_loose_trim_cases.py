"""Authored cubic trim images, independent of offset output coefficients."""
import copy
if __package__:
    from .offset_surface_loose_cases import BEZIER, case, polyline, trimmed, LOOSE_PILOT
else:
    from offset_surface_loose_cases import BEZIER, case, polyline, trimmed, LOOSE_PILOT

def split(points,t):
    rows=[points]
    while len(rows[-1])>1:
        rows.append([[(1-t)*a[k]+t*b[k]for k in range(3)]for a,b in zip(rows[-1],rows[-1][1:])])
    return [r[0]for r in rows],[r[-1]for r in reversed(rows)]

def restricted(points,a,b):
    return split(split(points,a)[1],(b-a)/(1-a))[0]

def boundary(a,b,reverse=False):
    controls=[p['point']for p in BEZIER['control_points']]
    # Collapse the tensor's other axis, then restrict each exact cubic isocurve.
    row=lambda v:[split([controls[j*4+i]for j in range(4)],v)[0][-1]for i in range(4)]
    column=lambda u:[split(controls[j*4:j*4+4],u)[0][-1]for j in range(4)]
    segments=[restricted(row(a),a,b),restricted(column(b),a,b),
              list(reversed(restricted(row(b),a,b))),list(reversed(restricted(column(a),a,b)))]
    uv=[[a,a],[b,a],[b,b],[a,b],[a,a]]
    if reverse:
        segments=[list(reversed(s))for s in reversed(segments)]
        uv=list(reversed(uv))
    points=segments[0]+[p for segment in segments[1:]for p in segment[1:]]
    spatial=dict(degree=3,control_points=[dict(point=p,weight=1.)for p in points],
        knots=[0.]*4+[1.]*3+[2.]*3+[3.]*3+[4.]*4)
    return dict(curve=spatial,parameter_curve=polyline([[u,v,0.]for u,v in uv]))

CURVED_TRIM=dict(type='brep',surface=copy.deepcopy(BEZIER),reversed=False,interior_uv=[.2,.2],
    boundaries=[boundary(.1,.9),boundary(.35,.65,True)])
SECOND=copy.deepcopy(BEZIER)
for p in SECOND['control_points']:p['point'][0]+=10.
LOOSE_TRIMS={
    'offset_lt_curved':case(CURVED_TRIM),
    'offset_lt_curved_solid':case(CURVED_TRIM,options=['Solid=Yes']),
    'offset_lt_curved_both_solid':case(CURVED_TRIM,options=['Solid=Yes','BothSides=Yes']),
    'offset_lt_plane_zero_solid':case(LOOSE_PILOT['offset_loose_plane']['curves'][0],options=['Solid=Yes',0.]),
    'offset_lt_trim_zero_solid':case(trimmed(),options=['Solid=Yes',0.]),
    'offset_lt_batch_both':dict(case(BEZIER,options=['BothSides=Yes']),curves=[BEZIER,SECOND]),
}
