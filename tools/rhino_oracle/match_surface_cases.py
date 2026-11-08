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

TOLERANCE_HOLDOUTS={}
for percent in [.00001,.05,1.,100.]:
    value=copy.deepcopy(FINAL['match_f_rational_refine_curvature'])
    value.update(angle_tolerance=.1,distance_tolerance=.001,curvature_tolerance=percent,audit=True)
    TOLERANCE_HOLDOUTS['match_tol_sdk_curvature_'+str(percent)]=value
for distance in [.01,.000001]:
    value=copy.deepcopy(FINAL['match_f_rational_refine_position'])
    value.update(angle_tolerance=.1,distance_tolerance=distance,curvature_tolerance=.05,audit=True)
    TOLERANCE_HOLDOUTS['match_tol_sdk_distance_'+str(distance)]=value
CASES.update(copy.deepcopy(TOLERANCE_HOLDOUTS))

TOLERANCE_SWEEP={}
for mode in ['Position','Curvature']:
    for distance in [1e-8,1e-6,1e-5,1e-4,.001,.01]:
        value=copy.deepcopy(FINAL['match_f_rational_refine_'+mode.lower()])
        value.update(angle_tolerance=.1,distance_tolerance=distance,curvature_tolerance=.05,audit=True)
        TOLERANCE_SWEEP['match_tol_sweep_'+mode.lower()+'_'+str(distance)]=value
for scale in [.1,10.]:
    for distance in [1e-6,.001]:
        value=copy.deepcopy(FINAL['match_f_rational_refine_position'])
        for source in value['sources']:
            for control in source['control_points']:control['point']=[x*scale for x in control['point']]
        value.update(angle_tolerance=.1,distance_tolerance=distance*scale,curvature_tolerance=.05,audit=True)
        TOLERANCE_SWEEP['match_tol_scaled_'+str(scale)+'_'+str(distance)]=value
CASES.update(copy.deepcopy(TOLERANCE_SWEEP))

CLOSEST_AUDIT={}
for kind,mode,distance in [('rational','Position',.01),('rational','Curvature',.001),('polynomial','Position',.01)]:
    value=copy.deepcopy(FINAL['match_f_'+kind+'_refine_'+mode.lower()])
    value.update(angle_tolerance=.1,distance_tolerance=distance,curvature_tolerance=.05,audit=True,surface_closest_audit=True)
    CLOSEST_AUDIT['match_closest_audit_'+kind+'_'+mode.lower()]=value
CASES.update(copy.deepcopy(CLOSEST_AUDIT))

LOCAL_CLOSEST_AUDIT=copy.deepcopy(CLOSEST_AUDIT)
LOCAL_CLOSEST_AUDIT={key.replace('closest_audit','local_closest_audit'):dict(value,local_closest_audit=True) for key,value in LOCAL_CLOSEST_AUDIT.items()}
CASES.update(copy.deepcopy(LOCAL_CLOSEST_AUDIT))

LOCAL_QUERY_AUDIT={key.replace('local_closest_audit','local_query_audit'):copy.deepcopy(value) for key,value in LOCAL_CLOSEST_AUDIT.items()}
CASES.update(copy.deepcopy(LOCAL_QUERY_AUDIT))

TOLERANCE_MIRRORS={}
for name,distance,angle,curvature in [
    ('tighter',1e-5,0.0017453292519943296,100.),
    ('loose',.001,0.017453292519943295,.01),
    ('document',.0001,1e-6,.05),
]:
    value=copy.deepcopy(FINAL['match_f_rational_refine_curvature'])
    value.update(distance_tolerance=distance,angle_tolerance=angle,curvature_tolerance=curvature,audit=True)
    TOLERANCE_MIRRORS['match_tol_mirror_'+name]=value
for kind in ['polynomial','rational']:
    value=copy.deepcopy(FINAL['match_f_'+kind+'_refine_position'])
    value['sources'][0]=chart_edit(value['sources'][1],'reverse_u')
    value.update(distance_tolerance=1e-6,angle_tolerance=.1,curvature_tolerance=.05,audit=True)
    TOLERANCE_MIRRORS['match_tol_already_matched_'+kind]=value
CASES.update(copy.deepcopy(TOLERANCE_MIRRORS))

INVERSE_QUERY_AUDIT={key.replace('closest_audit','inverse_query_audit'):dict(copy.deepcopy(value),inverse_surface_audit=True) for key,value in CLOSEST_AUDIT.items()}
CASES.update(copy.deepcopy(INVERSE_QUERY_AUDIT))

DEVIATION_QUERY_AUDIT={key.replace('closest_audit','deviation_query_audit'):dict(copy.deepcopy(value),curve_deviation_audit=True) for key,value in CLOSEST_AUDIT.items()}
GAP_SWEEP={}
for factor in [0.,.001,.01,.1,1.,10.]:
    value=copy.deepcopy(FINAL['match_f_rational_refine_position']);source,target=value['sources']
    for j in range(4):
        a=source['control_points'][j*4+3];b=target['control_points'][j*4]
        a['point']=[q+factor*(p-q)for p,q in zip(a['point'],b['point'])]
    value.update(distance_tolerance=1e-6,angle_tolerance=.1,curvature_tolerance=.05,audit=True)
    GAP_SWEEP['match_tol_gap_'+str(factor)]=value
CASES.update(copy.deepcopy(DEVIATION_QUERY_AUDIT));CASES.update(copy.deepcopy(GAP_SWEEP))
