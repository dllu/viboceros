"""Fixed MeshSplit scenes for open cuts, coplanar regions and projected curves."""
from copy import deepcopy
if __package__:
    from .mesh_self_intersect_cases import box,xy,wall,overlap,mesh,combine
else:
    from mesh_self_intersect_cases import box,xy,wall,overlap,mesh,combine


def moved(source,delta):
    result=deepcopy(source)
    result['vertices']=[[p[i]+delta[i]for i in range(3)]for p in result['vertices']]
    return result


CASES={}
for name,sources in [
    ('box_plane',[box,wall]),('open_sheets',[xy,wall]),
    ('disjoint',[box,moved(wall,[8.,0.,0.])]),
    ('closed_cutter',[box,moved(box,[2.,1.,1.])]),
    ('coplanar',[xy,overlap]),('identical_sheet',[xy,xy]),
    ('face_contact',[box,moved(wall,[-2.,0.,0.])]),
    ('partial_cut',[xy,mesh([[2.,1.,-1.],[2.,4.,-1.],[2.,4.,1.],[2.,1.,1.]],[[0,1,2,3]])]),
    ('brep_cutter',[box,dict(moved(box,[2.,1.,1.]),kind='brep_box')]),
    ('curve_cutter',[box,dict(kind='polyline',points=[[2.,-1.,0.],[2.,4.,0.]])]),
]:
    for pre in [False,True]:
        CASES['mesh_split_'+name+('_pre'if pre else '_post')]=dict(
            operation='split',preselect=pre,sources=deepcopy(sources),first=[0],second=[1])
CASES['mesh_split_two_cutters']=dict(operation='split',preselect=False,
    sources=deepcopy([box,wall,mesh([[-1.,2.,-1.],[5.,2.,-1.],[5.,2.,3.],[-1.,2.,3.]],[[0,1,2,3]])]),first=[0],second=[1,2])
CASES['mesh_split_two_targets']=dict(operation='split',preselect=False,
    sources=deepcopy([box,xy,wall]),first=[0,1],second=[2])
CASES['mesh_split_disconnected']=dict(operation='split',preselect=False,
    sources=[combine(box,moved(box,[8.,0.,0.])),deepcopy(wall)],first=[0],second=[1])
CASES['mesh_split_cancel_first']=dict(operation='split',preselect=False,
    sources=deepcopy([box,wall]),first=[0],second=[1],cancel=True)
CASES['mesh_split_cancel_second']=dict(operation='split',preselect=False,
    sources=deepcopy([box,wall]),first=[0],second=[1],cancel_second=True)
CONTROLS={}
for name,sources,keep,ngons,discard in [
    ('box_ngons_yes',[box,wall],True,True,True),
    ('box_ngons_no',[box,wall],True,False,True),
    ('partial_keep',[xy,mesh([[2.,1.,-1.],[2.,4.,-1.],[2.,4.,1.],[2.,1.,1.]],[[0,1,2,3]])],True,True,False),
    ('disjoint_keep',[box,moved(wall,[8.,0.,0.])],True,True,False),
    ('coplanar_keep',[xy,overlap],True,True,True),
    ('coplanar_drop',[xy,overlap],False,True,True),
    ('face_contact_drop',[box,moved(wall,[-2.,0.,0.])],False,True,True),
    ('identical_drop',[xy,xy],False,True,True),
    ('two_cutters',[box,wall,mesh([[-1.,2.,-1.],[5.,2.,-1.],[5.,2.,3.],[-1.,2.,3.]],[[0,1,2,3]])],True,True,True),
    ('mixed_cutter',[xy,combine(overlap,wall)],True,True,True),
    ('three_cutters',[box,wall,mesh([[-1.,2.,-1.],[5.,2.,-1.],[5.,2.,3.],[-1.,2.,3.]],[[0,1,2,3]]),
        mesh([[-1.,-1.,1.],[5.,-1.,1.],[5.,4.,1.],[-1.,4.,1.]],[[0,1,2,3]])],True,True,True),
]:
    CONTROLS['mesh_split_controls_'+name]=dict(operation='split',preselect=False,
        sources=deepcopy(sources),first=[0],second=list(range(1,len(sources))),
        arguments='_Tolerance=_Auto _CoplanarFaces=_{0} _CreateNgons=_{1} _DiscardUnsplitMeshes=_{2}'.format(
            'Keep'if keep else'Drop','Yes'if ngons else'No','Yes'if discard else'No'),
        capture_mesh_split_sdk=True,split_coplanar=keep,create_ngons=ngons,discard_unsplit=discard)
if __package__:
    from .mesh_split_policy_cases import cases as policy_cases
else:
    from mesh_split_policy_cases import cases as policy_cases
if __package__:
    from .mesh_split_selection_cases import cases as selection_cases
else:
    from mesh_split_selection_cases import cases as selection_cases
POLICY=policy_cases(box,xy,wall,overlap,combine,moved)
SELECTION=selection_cases(box,xy,wall,overlap)
if __package__:
    from .mesh_split_channel_cases import cases as channel_cases
else:
    from mesh_split_channel_cases import cases as channel_cases
CHANNELS=channel_cases(box,xy,wall,moved)
if __package__:
    from .mesh_split_ngon_policy_cases import cases as ngon_policy_cases
else:
    from mesh_split_ngon_policy_cases import cases as ngon_policy_cases
NGON_POLICY=ngon_policy_cases(CHANNELS)
FAMILIES=dict(discovery=CASES,controls=CONTROLS,policy=POLICY,selection=SELECTION,channels=CHANNELS,ngon_policy=NGON_POLICY)
ALL_CASES=dict(CASES,**CONTROLS)
ALL_CASES.update(POLICY)
ALL_CASES.update(SELECTION)
ALL_CASES.update(CHANNELS)
ALL_CASES.update(NGON_POLICY)


def request(family='discovery'):
    return dict(protocol_version=1,iterations=1,
        tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),
        operations=[dict(op='mesh_split_command',id=name,case=name)for name in FAMILIES[family]])


def validate(operation):
    if not (isinstance(operation,dict)and set(operation)=={'op','id','case'}
            and operation.get('op')=='mesh_split_command'
            and isinstance(operation.get('case'),str)and operation['case']in ALL_CASES
            and operation['id']==operation['case']):
        raise ValueError('fixed MeshSplit command recipe required')
