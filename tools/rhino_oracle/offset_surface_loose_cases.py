"""Authored control nets for public Loose OffsetSrf observations."""
import copy
import math


def surface(rows, degree=(3, 3), knots=None, weights=None):
    nv=len(rows); nu=len(rows[0])
    if knots is None:
        knots=([0.]*(degree[0]+1)+[1.]*(degree[0]+1),
               [0.]*(degree[1]+1)+[1.]*(degree[1]+1))
    controls=[dict(point=list(p),weight=weights[j][i] if weights else 1.)
              for j,row in enumerate(rows) for i,p in enumerate(row)]
    return dict(type='surface',degree_u=degree[0],degree_v=degree[1],
                control_point_count_u=nu,control_point_count_v=nv,
                control_points=controls,knots_u=list(knots[0]),knots_v=list(knots[1]))


HEIGHTS=[[0.,.2,-.1,.3],[.1,.8,.7,-.2],[0.,.6,1.1,.1],[.2,.1,0.,-.2]]
BEZIER=surface([[[i*1.7,j*1.2,z]for i,z in enumerate(row)]for j,row in enumerate(HEIGHTS)])
NONUNIFORM=surface([[[x,y,(i-j)*.17+(i*j)*.12]for i,x in enumerate([0.,.7,2.8,4.])]
                    for j,y in enumerate([0.,1.,3.])],degree=(2,2),
                   knots=([2.,2.,2.,3.,7.,7.,7.],[-3.,-3.,-3.,5.,5.,5.]))
RATIONAL=copy.deepcopy(NONUNIFORM)
for i,p in enumerate(RATIONAL['control_points']): p['weight']=[1.,1.4,.8,1.2,1.1,.7,1.6,1.,1.3,1.,.9,1.5][i]


def case(source, distance=.5, options=None):
    actions=['Corner=Sharp','Solid=Yes','DeleteInput=No','Solid=No','BothSides=No',
             'Loose=Yes','Distance',abs(distance)]
    if distance<0: actions.append(distance)
    return dict(curves=[copy.deepcopy(source)],view='Top',preselected=True,
                pending_redo=True,capture_undo=True,actions=actions+(options or [])+['Enter'])


LOOSE_PILOT={
    'offset_loose_plane':case(surface([[[0.,0.,0.],[4.,0.,0.]],[[0.,3.,0.],[4.,3.,0.]]],degree=(1,1))),
    'offset_loose_saddle':case(surface([[[0.,0.,0.],[4.,0.,0.]],[[0.,3.,0.],[4.,3.,1.]]],degree=(1,1))),
    'offset_loose_bezier':case(BEZIER),
    'offset_loose_nonuniform':case(NONUNIFORM),
    'offset_loose_rational':case(RATIONAL),
    'offset_loose_negative':case(BEZIER,distance=-.5),
    'offset_loose_both':case(BEZIER,options=['BothSides=Yes']),
    'offset_loose_solid':case(BEZIER,options=['Solid=Yes']),
    'offset_loose_polysurface':case(dict(type='box_brep',min=[0.,0.,0.],max=[4.,3.,2.])),
}

def polyline(points):
    return dict(degree=1,control_points=[dict(point=p,weight=1.)for p in points],
                knots=[0.]+[float(i)for i in range(len(points))]+[float(len(points)-1)])

SADDLE=LOOSE_PILOT['offset_loose_saddle']['curves'][0]
def trimmed(reversed_face=False):
    loops=[[[.1,.1],[.9,.1],[.9,.9],[.1,.9],[.1,.1]],
           [[.35,.35],[.35,.65],[.65,.65],[.65,.35],[.35,.35]]]
    return dict(type='brep',surface=copy.deepcopy(SADDLE),reversed=reversed_face,
        interior_uv=[.2,.2],boundaries=[dict(parameter_curve=polyline([[u,v,0.]for u,v in uv]),
            curve=polyline([[4*u,3*v,u*v]for u,v in uv]))for uv in loops])

def periodic(torus=False):
    nu,nv=8,6 if torus else 2
    points=[]
    for j in range(nv+(3 if torus else 0)):
        b=2*math.pi*(j%nv)/nv
        points.append([[(4+math.cos(b) if torus else 4)*math.cos(2*math.pi*(i%nu)/nu),
                        (4+math.cos(b) if torus else 4)*math.sin(2*math.pi*(i%nu)/nu),
                        math.sin(b) if torus else 3*j]for i in range(nu+3)])
    return surface(points,degree=(3,3 if torus else 1),
        knots=(list(map(float,range(-3,nu+4))),list(map(float,range(-3,nv+4)))if torus else [0.,0.,1.,1.]))

def round_surface(sphere=False):
    circle=[(1,0),(1,1),(0,1),(-1,1),(-1,0),(-1,-1),(0,-1),(1,-1),(1,0)]
    meridian=[(0,4),(4,4),(4,0),(4,-4),(0,-4)]if sphere else [(4,0),(4,3)]
    points=[[[r*x,r*y,z]for x,y in circle]for r,z in meridian]
    weights=[[(2**-.5 if i%2 else 1.)*(2**-.5 if sphere and j%2 else 1.)
              for i in range(9)]for j in range(len(meridian))]
    return surface(points,degree=(2,2 if sphere else 1),weights=weights,
        knots=([0.,0.,0.,1.,1.,2.,2.,3.,3.,4.,4.,4.],
               [0.,0.,0.,1.,1.,2.,2.,2.]if sphere else [0.,0.,1.,1.]))

CREASE=surface([[[i,j,z+.13*i*j]for i,z in enumerate([0.,.3,.6,.2,.1])]for j in range(3)],
    degree=(2,2),knots=([0.,0.,0.,.5,.5,1.,1.,1.],[0.,0.,0.,1.,1.,1.]))
HIGH=surface([[[i*.7,j*1.1,.1*math.sin(i+j)]for i in range(6)]for j in range(3)],degree=(5,2))
LOOSE_HOLDOUTS={
    'offset_l_trimmed':case(trimmed()),
    'offset_l_trimmed_reversed':case(trimmed(True)),
    'offset_l_trimmed_solid':case(trimmed(),options=['Solid=Yes']),
    'offset_l_solid_delete':case(RATIONAL,options=['Solid=Yes','DeleteInput=Yes']),
    'offset_l_both_solid':case(BEZIER,options=['BothSides=Yes','Solid=Yes']),
    'offset_l_flip':case(BEZIER,options=['FlipAll']),
    'offset_l_zero':case(BEZIER,distance=0.,options=[0.]),
    'offset_l_cancel':dict(case(BEZIER),actions=case(BEZIER)['actions'][:-1]+['Cancel']),
    'offset_l_periodic_u':case(periodic()),
    'offset_l_periodic_uv':case(periodic(True),distance=.2),
    'offset_l_closed_rational':case(round_surface()),
    'offset_l_sphere_poles':case(round_surface(True)),
    'offset_l_crease':case(CREASE),
    'offset_l_degree_five':case(HIGH),
}

def closed_case(source,distance=.5):
    value=case(source,distance)
    value['actions'].remove('DeleteInput=No')
    return value

LOOSE_FINAL={
    'offset_lf_periodic_uv':closed_case(periodic(True),.2),
    'offset_lf_sphere_poles':closed_case(round_surface(True)),
    'offset_lf_postselect':dict(case(BEZIER),preselected=False,
        actions=[{'curve':0},'Enter']+case(BEZIER)['actions']),
    'offset_lf_postselect_cancel':dict(case(BEZIER),preselected=False,
        actions=[{'curve':0},'Enter']+case(BEZIER)['actions'][:-1]+['Cancel']),
    'offset_lf_zero_solid':case(BEZIER,options=['Solid=Yes',0.]),
    'offset_lf_zero_both_solid':case(BEZIER,options=['Solid=Yes','BothSides=Yes',0.]),
    'offset_lf_zero_solid_delete':case(BEZIER,options=['Solid=Yes','DeleteInput=Yes',0.]),
    'offset_lf_units':case(RATIONAL,options=[{'number':'.125cm'}]),
    'offset_lf_toggle':case(BEZIER,options=['Loose=No','Loose=Yes']),
}

LOOSE_CLOSED={
    'offset_lc_sphere_solid':closed_case(round_surface(True)),
    'offset_lc_periodic_solid':closed_case(periodic(True),.2),
    'offset_lc_periodic_cylinder_solid':case(periodic(),options=['Solid=Yes']),
    'offset_lc_rational_cylinder_solid':case(round_surface(),options=['Solid=Yes']),
}
for name in ['offset_lc_sphere_solid','offset_lc_periodic_solid']:
    LOOSE_CLOSED[name]['actions'].insert(-1,'Solid=Yes')

# Prime the hidden DeleteInput preference through a surface where its option
# is visible. The following closed-shell recipes state that authored context.
LOOSE_SHELLS={}
for delete in [False,True]:
    label='delete'if delete else'keep'
    primer=case(LOOSE_PILOT['offset_loose_plane']['curves'][0],options=['Solid=Yes','DeleteInput='+('Yes'if delete else'No')])
    primer['actions'][-1]='Cancel'
    LOOSE_SHELLS['offset_ls_prime_'+label]=primer
    for name,source,distance in [('sphere',round_surface(True),.5),('periodic',periodic(True),.2)]:
        value=closed_case(source,distance);value['actions'].insert(-1,'Solid=Yes')
        value['initial_delete_input']=delete
        LOOSE_SHELLS['offset_ls_'+name+'_'+label]=value
