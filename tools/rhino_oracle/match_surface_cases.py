"""Authored, independently varied surface-edge continuity experiments."""
import copy


def surface(target=False, degree=3, rational=False):
    controls=[]
    for j in range(4):
        for i in range(degree+1):
            u=i/float(degree);v=j/3.
            x=1.12+u if target else u
            z=(.2+.4*u+.3*u*u if target else -.2+.15*u+.1*u*u)+.15*v+.04*u*v
            controls.append(dict(point=[x,v,z],weight=1.+.1*((i+j)%3) if rational else 1.))
    return dict(type='surface',degree_u=degree,degree_v=3,
        control_point_count_u=degree+1,control_point_count_v=4,control_points=controls,
        knots_u=[0.]*(degree+1)+[1.]*(degree+1),knots_v=[0.]*4+[1.]*4)


def case(continuity='Position',other_end='None',**changes):
    value=dict(sources=[surface(),surface(True)],side='East',target_side='West',
        continuity=continuity,other_end=other_end,iso='Automatic',average=False,
        closest_points=False,reverse_match=False,reverse_target=False,refine=False)
    value.update(changes)
    return copy.deepcopy(value)


PILOT={}
for continuity in ['Position','Tangency','Curvature']:
    for preserve in ['None','Position','Tangency','Curvature']:
        PILOT['match_'+continuity.lower()+'_'+preserve.lower()]=case(continuity,preserve)
CASES=copy.deepcopy(PILOT)


def request(names=None):
    return dict(protocol_version=1,iterations=1,
        tolerance=dict(absolute=1e-6,relative=1e-9,angular=1e-6),
        operations=[dict(op='match_surface_sdk',id=name,case=name) for name in (names if names is not None else CASES)])

HOLDOUTS={}
for name,changes in [
    ('aligned',dict(reverse_match=True)),
    ('rational',dict(sources=[surface(rational=True),surface(True,rational=True)])),
    ('linear',dict(sources=[surface(degree=1),surface(True,degree=1)])),
    ('quadratic',dict(sources=[surface(degree=2),surface(True,degree=2)])),
    ('quartic',dict(sources=[surface(degree=4),surface(True,degree=4)])),
    ('unequal_cross',dict(sources=[surface(degree=2),surface(True,degree=4)])),
    ('average',dict(average=True)),
    ('average_reverse',dict(average=True,reverse_target=True)),
    ('closest',dict(closest_points=True)),
    ('refine',dict(refine=True)),
]:
    for continuity in ['Position','Tangency','Curvature']:
        HOLDOUTS['match_h_'+name+'_'+continuity.lower()]=case(continuity,**changes)
for iso in ['Preserve','MatchTarget','Perpendicular']:
    for continuity in ['Tangency','Curvature']:
        value=case(continuity,iso=iso)
        for control in value['sources'][1]['control_points']:
            control['point'][0]+=.2*control['point'][1]
        HOLDOUTS['match_h_iso_'+iso.lower()+'_'+continuity.lower()]=value
CASES.update(copy.deepcopy(HOLDOUTS))

FINAL={}
for kind in ['polynomial','rational']:
    for continuity in ['Position','Tangency','Curvature']:
        value=case(continuity,reverse_match=True,
            sources=[surface(rational=kind=='rational'),surface(True,rational=kind=='rational')])
        for index,source in enumerate(value['sources']):
            for c in source['control_points']:
                x,y,z=c['point'];c['point']=[x+.12*y,y,z+.25*y*y+.08*y*y*y]
            source.update(domain_u=[2.,5.] if index==0 else [-4.,-2.],domain_v=[-3.,-1.] if index==0 else [4.,9.])
        FINAL['match_f_'+kind+'_'+continuity.lower()]=value
        refined=copy.deepcopy(value);refined['refine']=True
        FINAL['match_f_'+kind+'_refine_'+continuity.lower()]=refined
for continuity in ['Position','Tangency','Curvature']:
    value=case(continuity,other_end='Curvature')
    source=value['sources'][0];source['control_point_count_u']=5;source['knots_u']=[0.]*4+[.4]+[1.]*4
    source['control_points']=[]
    for j in range(4):
        for i in range(5):
            u=i/4.;v=j/3.;source['control_points'].append(dict(point=[u,v,-.2+.2*u+.06*u*u+.15*v],weight=1.))
    FINAL['match_f_multi_'+continuity.lower()]=value
value=case('Curvature',reverse_match=True)
source=value['sources'][0];source['degree_v']=2;source['control_point_count_v']=3
source['knots_v']=[0.]*3+[1.]*3;source['control_points']=source['control_points'][:12]
for c in value['sources'][1]['control_points']:
    c['point'][2]+=.3*c['point'][1]**3
FINAL['match_f_transverse_basis']=value
CASES.update(copy.deepcopy(FINAL))

REFINEMENT={}
for continuity in ['Position','Tangency','Curvature']:
    for angle in [1e-5,1e-6,1e-7]:
        value=copy.deepcopy(FINAL['match_f_rational_refine_'+continuity.lower()])
        value.update(audit=True,angle_tolerance=angle)
        REFINEMENT['match_a_'+continuity.lower()+'_'+str(angle)]=value
CASES.update(copy.deepcopy(REFINEMENT))


def chart_edit(surface,operation):
    value=copy.deepcopy(surface);nu=value['control_point_count_u'];nv=value['control_point_count_v']
    controls=value['control_points']
    if operation=='transpose':
        value['control_points']=[controls[j*nu+i] for i in range(nu) for j in range(nv)]
        for key in ['degree','control_point_count','knots','domain']:
            a,b=key+'_u',key+'_v'
            if a in value:value[a],value[b]=value[b],value[a]
    else:
        axis=operation[-1]
        value['control_points']=([controls[j*nu+i] for j in range(nv) for i in reversed(range(nu))]
            if axis=='u' else [controls[j*nu+i] for j in reversed(range(nv)) for i in range(nu)])
        knots=value['knots_'+axis];value['knots_'+axis]=[knots[0]+knots[-1]-k for k in reversed(knots)]
    return value


ORIENTED={}
for name,source_op,target_op,source_side,target_side in [
    ('first_west','reverse_u',None,'West','West'),
    ('first_north','transpose',None,'North','West'),
    ('first_south','transpose_reverse_v',None,'South','West'),
    ('target_east',None,'reverse_u','East','East'),
    ('target_south',None,'transpose','East','South'),
    ('target_north',None,'transpose_reverse_v','East','North'),
    ('first_reversed_v','reverse_v',None,'East','West'),
    ('both_transpose','transpose','transpose','North','South'),
]:
    value=case('Curvature',other_end='Curvature',side=source_side,target_side=target_side)
    for index,operation in enumerate([source_op,target_op]):
        if operation:
            for op in (['transpose','reverse_v'] if operation=='transpose_reverse_v' else [operation]):
                value['sources'][index]=chart_edit(value['sources'][index],op)
    ORIENTED['match_o_'+name]=value
# The first final launch completed its immutable response, but its tool handle
# was not retained. Repeat these recipes under a launch with an exit receipt.
for key,value in FINAL.items():ORIENTED[key.replace('match_f_','match_v_')]=copy.deepcopy(value)
CASES.update(copy.deepcopy(ORIENTED))
