"""Independent native TriangulateNonPlanarQuads command inputs."""
from copy import deepcopy
WARP=[[0.,0.,0.],[2.,0.,0.],[2.,1.,0.],[0.,1.,.4]]
FLAT=[[0.,0.,0.],[2.,0.,0.],[2.,1.,0.],[0.,1.,0.]]


def scene(vertices,arguments=''):
    return dict(command='TriangulateNonPlanarQuads',whole=True,preselected=True,
        sources=[dict(kind='mesh',vertices=deepcopy(vertices),faces=[[0,1,2,3]])],
        command_arguments=arguments)


DISCOVERY={
    'nonplanar_command_default_warp':scene(WARP),
    'nonplanar_command_default_flat':scene(FLAT),
    'nonplanar_command_default_triangles':scene(WARP),
}
DISCOVERY['nonplanar_command_default_triangles']['sources'][0]['faces']=[[0,1,2],[0,2,3]]
OPTIONS={
    'nonplanar_command_angle_default':scene(WARP,'_SelectBy=_Angle'),
    'nonplanar_command_both_default':scene(WARP,'_SelectBy=_Both'),
    'nonplanar_command_explicit_distance':scene(WARP,'_SelectBy=_Distance _Distance=0.25 _SplitMethod=_LongestDiagonal'),
    'nonplanar_command_remembered_distance':scene(WARP),
}
SYNTAX={
    'nonplanar_command_select_menu':scene(WARP,'_SelectBy'),
    'nonplanar_command_split_menu':scene(WARP,'_SplitMethod'),
    'nonplanar_command_distance_separate':scene(WARP,'_Distance 0.25'),
    'nonplanar_command_angle_separate':scene(WARP,'_SelectBy=_Angle _Angle 30'),
    'nonplanar_command_combined_name':scene(WARP,'_SelectBy=_DistanceAndAngle'),
    'nonplanar_command_selection_cancel':dict(scene(WARP),whole=False,preselected=False,cancel=True),
}
MAIN={
    'nonplanar_command_main_defaults':scene(WARP),
    'nonplanar_command_distance_high':scene(WARP,'_Distance 0.25'),
    'nonplanar_command_remembered_high':scene(WARP),
    'nonplanar_command_distance_low':scene(WARP,'_Distance 0.01 _SplitMethod _LongestDiagonal'),
    'nonplanar_command_remembered_low':scene(WARP),
    'nonplanar_command_angle_high':scene(WARP,'_SelectBy=_Angle _Angle 30 _SplitMethod _MinimumArea'),
    'nonplanar_command_remembered_angle':scene(WARP),
    'nonplanar_command_angle_zero':scene(WARP,'_Angle 0 _SplitMethod _MaximumArea'),
    'nonplanar_command_minimum_angle':scene(WARP,'_SplitMethod _MinimumAngle'),
    'nonplanar_command_maximum_angle':scene(WARP,'_SplitMethod _MaximumAngle'),
    'nonplanar_command_shortest':scene(WARP,'_SplitMethod _ShortestDiagonal'),
    'nonplanar_command_cancel_options':dict(scene(WARP,'_Angle 5 _SplitMethod _LongestDiagonal'),cancel=True),
    'nonplanar_command_after_cancel':scene(WARP),
    'nonplanar_command_toggle':scene(WARP,'_SelectBy'),
    'nonplanar_command_first_selection':dict(scene(WARP,'_All _Enter _SelectBy=_Distance _Distance 0.01'),whole=False,preselected=False),
    'nonplanar_command_flat':scene(FLAT),
    'nonplanar_command_triangles':scene(WARP),
    'nonplanar_command_channels_ngon':scene(WARP,'_Distance 0.01'),
    'nonplanar_command_mixed':scene(WARP),
    'nonplanar_command_cancel_selection':dict(scene(WARP),whole=False,preselected=False,cancel=True),
}
MAIN['nonplanar_command_triangles']['sources'][0]['faces']=[[0,1,2],[0,2,3]]
MAIN['nonplanar_command_channels_ngon']['sources'][0].update(
    colors=[[255,0,0,255],[0,255,0,128],[0,0,255,0],[90,80,70,255]],
    texture_coordinates=[[0.,0.],[1.,0.],[1.,1.],[0.,1.]],normals=[[1.,0.,0.]]*4,
    ngons=[dict(vertices=[0,1,2,3],faces=[0])])
MAIN['nonplanar_command_channels_ngon']['mesh_channels']=True
MAIN['nonplanar_command_mixed']['sources'].append(dict(kind='subd',vertices=[[x+3,y,z]for x,y,z in FLAT],faces=[[0,1,2,3]]))
SELECTION={
    'nonplanar_command_postselect_high':dict(scene(WARP,'_All _Distance 0.25'),whole=False,preselected=False),
    'nonplanar_command_postselect_low':dict(scene(WARP,'_All _Distance 0.01'),whole=False,preselected=False),
}
CASES=dict(DISCOVERY);CASES.update(OPTIONS);CASES.update(SYNTAX);CASES.update(MAIN);CASES.update(SELECTION)


def request(options=False,syntax=False,main=False,selection=False):
    return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),
        operations=[dict(op='nonplanar_quad_command',id=k,case=k)for k in (SELECTION if selection else MAIN if main else SYNTAX if syntax else OPTIONS if options else DISCOVERY)])


def validate(op):
    try:valid=isinstance(op,dict)and set(op)=={'op','id','case'}and op['op']=='nonplanar_quad_command'and op['case']in CASES and op['id']==op['case']
    except TypeError:valid=False
    if not valid:raise ValueError('fixed nonplanar quad command recipe required')
