"""Independent two-sided surface matches, with explicit traversal choices."""
import copy
if __package__:
    from .match_surface_cases import FINAL,case,surface,chart_edit
else:
    from match_surface_cases import FINAL,case,surface,chart_edit

PILOT={}
for kind in ['polynomial','rational']:
    for mode in ['Position','Tangency','Curvature']:
        for first,second in [(False,False),(False,True),(True,False),(True,True)]:
            value=copy.deepcopy(FINAL['match_f_'+kind+'_'+mode.lower()])
            value.update(average=True,reverse_match=first,reverse_target=second)
            key='average_'+kind+'_'+mode.lower()+'_'+str(int(first))+str(int(second))
            PILOT[key]=value
CASES=copy.deepcopy(PILOT)

HOLDOUTS={}
for mode in ['Position','Tangency']:
    base=copy.deepcopy(FINAL['match_f_rational_'+mode.lower()])
    base.update(average=True,reverse_match=True,reverse_target=False)
    for label,changes in [
        ('keep',dict(iso='Preserve')),('perpendicular',dict(iso='Perpendicular')),
        ('target_iso',dict(iso='MatchTarget')),('preserve_position',dict(other_end='Position')),
        ('preserve_tangency',dict(other_end='Tangency')),('preserve_curvature',dict(other_end='Curvature')),
        ('closest',dict(closest_points=True)),('refine',dict(refine=True)),
    ]:
        value=copy.deepcopy(base);value.update(changes)
        HOLDOUTS['average_h_'+mode.lower()+'_'+label]=value
    for a,b in [(1,3),(2,5)]:
        HOLDOUTS['average_h_'+mode.lower()+'_degrees_'+str(a)+'_'+str(b)]=case(mode,
            sources=[surface(degree=a,rational=True),surface(True,degree=b,rational=True)],
            average=True,reverse_match=True,reverse_target=False)
    for label,a,b,side,target_side in [
        ('source_west','reverse_u',None,'West','West'),
        ('source_north','transpose',None,'North','West'),
        ('source_south','transpose_reverse_v',None,'South','West'),
        ('target_east',None,'reverse_u','East','East'),
        ('target_south',None,'transpose','East','South'),
        ('target_north',None,'transpose_reverse_v','East','North'),
        ('source_reverse_v','reverse_v',None,'East','West'),
        ('both_transpose','transpose','transpose','North','South'),
    ]:
        value=copy.deepcopy(base);value.update(side=side,target_side=target_side)
        for i,operation in enumerate([a,b]):
            if operation:
                for op in (['transpose','reverse_v']if operation=='transpose_reverse_v'else[operation]):
                    value['sources'][i]=chart_edit(value['sources'][i],op)
        HOLDOUTS['average_h_'+mode.lower()+'_'+label]=value
CASES.update(copy.deepcopy(HOLDOUTS))

# Translating one complete input leaves its jets unchanged, isolating any
# positional coupling in the curvature control row.
CURVATURE_CONTEXTS={}
for kind in ['polynomial','rational']:
    for name,shift in [('base',[0.,0.,0.]),('shift_x',[.2,0.,0.]),('shift_y',[0.,.2,0.]),
                       ('shift_z',[0.,0.,.2]),('shift_xyz',[.03,.11,-.07])]:
        value=copy.deepcopy(FINAL['match_f_'+kind+'_curvature'])
        value.update(average=True,reverse_match=True,reverse_target=False)
        for c in value['sources'][0]['control_points']:c['point']=[p+d for p,d in zip(c['point'],shift)]
        CURVATURE_CONTEXTS['average_curvature_context_'+kind+'_'+name]=value
    value=copy.deepcopy(FINAL['match_f_'+kind+'_curvature'])
    value.update(average=True,reverse_match=True,reverse_target=False)
    for j in range(4):
        value['sources'][0]['control_points'][j*4+3]['point']=value['sources'][1]['control_points'][j*4]['point'][:]
    CURVATURE_CONTEXTS['average_curvature_context_'+kind+'_zero_gap']=value
CASES.update(copy.deepcopy(CURVATURE_CONTEXTS))

# Independent changes of the second row leave point and first derivatives fixed.
CURVATURE_ROWS={}
for kind in ['polynomial','rational']:
    for half,column in [(0,1),(1,2)]:
        for axis,label in enumerate('xyz'):
            value=copy.deepcopy(FINAL['match_f_'+kind+'_curvature'])
            value.update(average=True,reverse_match=True,reverse_target=False)
            for j in range(4):value['sources'][half]['control_points'][j*4+column]['point'][axis]+=.2
            CURVATURE_ROWS['average_curvature_rows_'+kind+'_'+str(half)+'_'+label]=value
CASES.update(copy.deepcopy(CURVATURE_ROWS))

CURVATURE_HOLDOUTS={}
for name,recipe in HOLDOUTS.items():
    if name.startswith('average_h_tangency_') and not recipe['refine']:
        value=copy.deepcopy(recipe);value['continuity']='Curvature'
        CURVATURE_HOLDOUTS[name.replace('average_h_tangency_','average_curvature_h_')]=value
for name in ['match_f_multi_curvature','match_f_transverse_basis']:
    value=copy.deepcopy(FINAL[name]);value.update(continuity='Curvature',average=True,reverse_match=True,reverse_target=False)
    CURVATURE_HOLDOUTS[name.replace('match_f_','average_curvature_h_')]=value
CASES.update(copy.deepcopy(CURVATURE_HOLDOUTS))

CURVATURE_NONUNIFORM={}
for rational in [False,True]:
    for preserve in ['None','Tangency','Curvature']:
        for knot in [.2,.7]:
            value=copy.deepcopy(FINAL['match_f_multi_curvature'])
            value.update(other_end=preserve,average=True,reverse_match=True,reverse_target=False)
            value['sources'][0]['knots_u'][4]=knot
            for j in range(4):
                for i in range(5):
                    c=value['sources'][0]['control_points'][j*5+i]
                    c['point'][2]+=.1*(j/3.)**2
                    if rational:c['weight']=1.+.1*((i+2*j)%3)
            value['sources'][1]=surface(True,rational=rational)
            key='average_curvature_nonuniform_'+('rational'if rational else'polynomial')+'_'+preserve.lower()+'_'+str(knot)
            CURVATURE_NONUNIFORM[key]=value
CASES.update(copy.deepcopy(CURVATURE_NONUNIFORM))

REFINEMENT_PILOT={}
for kind in ['polynomial','rational']:
    for mode in ['Position','Tangency','Curvature']:
        for first,second in [(False,False),(False,True),(True,False),(True,True)]:
            value=copy.deepcopy(FINAL['match_f_'+kind+'_'+mode.lower()])
            value.update(average=True,refine=True,reverse_match=first,reverse_target=second,settings_audit=True)
            key='average_refine_'+kind+'_'+mode.lower()+'_'+str(int(first))+str(int(second))
            REFINEMENT_PILOT[key]=value
        value=copy.deepcopy(FINAL['match_f_'+kind+'_'+mode.lower()])
        value.update(average=True,refine=True,reverse_match=True,reverse_target=False,settings_audit=True,
                     distance_tolerance=10.,angle_tolerance=1.,curvature_tolerance=100.)
        REFINEMENT_PILOT['average_refine_'+kind+'_'+mode.lower()+'_relaxed']=value
CASES.update(copy.deepcopy(REFINEMENT_PILOT))

REFINEMENT_STEPS={}
for kind in ['polynomial','rational']:
    for mode in ['Tangency','Curvature']:
        for first,second in [(False,False),(False,True),(True,True)]:
            value=copy.deepcopy(FINAL['match_f_'+kind+'_'+mode.lower()])
            value.update(average=True,reverse_match=first,reverse_target=second,settings_audit=True,average_pass_audit=True)
            REFINEMENT_STEPS['average_refine_steps_'+kind+'_'+mode.lower()+'_'+str(int(first))+str(int(second))]=value
CASES.update(copy.deepcopy(REFINEMENT_STEPS))

REFINEMENT_ISO={}
for kind in ['polynomial','rational']:
    for mode in ['Tangency','Curvature']:
        for first,second in [(False,False),(True,False)]:
            for iso in ['MatchTarget','Perpendicular','Preserve']:
                value=copy.deepcopy(FINAL['match_f_'+kind+'_'+mode.lower()])
                value.update(average=True,refine=True,reverse_match=first,reverse_target=second,settings_audit=True,iso=iso)
                REFINEMENT_ISO['average_refine_iso_'+kind+'_'+mode.lower()+'_'+str(int(first))+str(int(second))+'_'+iso.lower()]=value
CASES.update(copy.deepcopy(REFINEMENT_ISO))

REFINEMENT_TOLERANCES={}
for mode in ['Tangency','Curvature']:
    for first,second in [(False,False),(False,True),(True,False),(True,True)]:
        for angle in [.001,1.,3.]:
            value=copy.deepcopy(FINAL['match_f_polynomial_'+mode.lower()])
            value.update(average=True,refine=True,reverse_match=first,reverse_target=second,settings_audit=True,angle_tolerance=angle)
            REFINEMENT_TOLERANCES['average_refine_tolerance_'+mode.lower()+'_'+str(int(first))+str(int(second))+'_'+str(angle)]=value
for flags in ['00','11']:
    for curvature in [1e-5,100.]:
        value=copy.deepcopy(REFINEMENT_PILOT['average_refine_polynomial_curvature_'+flags])
        value['curvature_tolerance']=curvature
        REFINEMENT_TOLERANCES['average_refine_tolerance_curvature_'+flags+'_percent_'+str(curvature)]=value
CASES.update(copy.deepcopy(REFINEMENT_TOLERANCES))

REFINEMENT_PARAMETERS={}
for kind in ['polynomial','rational']:
    for mode in ['Tangency','Curvature']:
        value=copy.deepcopy(REFINEMENT_STEPS['average_refine_steps_'+kind+'_'+mode.lower()+'_00'])
        value['boundary_parameter_audit']=True
        REFINEMENT_PARAMETERS['average_refine_parameters_'+kind+'_'+mode.lower()]=value
CASES.update(copy.deepcopy(REFINEMENT_PARAMETERS))

REFINEMENT_EDGE_PARAMETERS={}
for name,recipe in REFINEMENT_PARAMETERS.items():
    value=copy.deepcopy(recipe);value['edge_parameter_audit']=True
    REFINEMENT_EDGE_PARAMETERS[name.replace('refine_parameters_','refine_edge_parameters_')]=value
CASES.update(copy.deepcopy(REFINEMENT_EDGE_PARAMETERS))

REFINEMENT_SIGNS={}
for mode in ['Tangency','Curvature']:
    for shift in [-.5,-.2,0.,.2,.5,1.]:
        value=copy.deepcopy(FINAL['match_f_polynomial_'+mode.lower()])
        value.update(average=True,refine=True,reverse_match=True,reverse_target=False,settings_audit=True)
        for c in value['sources'][0]['control_points']:c['point'][2]+=shift
        REFINEMENT_SIGNS['average_refine_signs_'+mode.lower()+'_'+str(shift)]=value
CASES.update(copy.deepcopy(REFINEMENT_SIGNS))

POSITION_REFINEMENT={}
for name,recipe in HOLDOUTS.items():
    if name.startswith('average_h_position_') and not recipe['refine']:
        value=copy.deepcopy(recipe);value.update(refine=True,settings_audit=True)
        POSITION_REFINEMENT[name.replace('average_h_position_','average_position_refine_')]=value
for name in ['match_f_multi_position','match_f_transverse_basis']:
    value=copy.deepcopy(FINAL[name]);value.update(continuity='Position',average=True,refine=True,
        reverse_match=True,reverse_target=False,settings_audit=True)
    POSITION_REFINEMENT[name.replace('match_f_','average_position_refine_')]=value
for first,second in [(True,False),(False,True)]:
    for distance in [1e-3,1e-5,1e-8]:
        value=copy.deepcopy(FINAL['match_f_rational_position'])
        value.update(average=True,refine=True,reverse_match=first,reverse_target=second,
            distance_tolerance=distance,settings_audit=True)
        for j,weight in enumerate([.8,1.6,1.1,1.4]):
            for i in range(4):value['sources'][1]['control_points'][j*4+i]['weight']=weight
        key='average_position_refine_weights_'+str(int(first))+str(int(second))+'_'+str(distance)
        POSITION_REFINEMENT[key]=value
value=case('Position',other_end='Curvature',sources=[surface(degree=1,rational=True),surface(True,rational=True)],
    average=True,refine=True,reverse_match=True,reverse_target=False,settings_audit=True)
POSITION_REFINEMENT['average_position_refine_short_preserved']=value
CASES.update(copy.deepcopy(POSITION_REFINEMENT))


def request(names=None):
    return dict(protocol_version=1,iterations=1,
        tolerance=dict(absolute=1e-6,relative=1e-9,angular=1e-6),
        operations=[dict(op='average_surface_sdk',id=name,case=name)for name in (CASES if names is None else names)])
