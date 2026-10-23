"""Independent mesh rebuild inputs and channel-preservation command recipes."""
from copy import deepcopy
POINTS=[[0.,0.,0.],[2.,0.,0.],[2.,1.,0.],[0.,1.,0.]]
COLORS=[[255,0,0,0],[0,255,0,127],[0,0,255,255],[90,80,70,0]]
UV=[[0.,0.],[1.,0.],[1.,1.],[0.,1.]]


def mesh(vertices=POINTS,faces=None,options=None,**extra):
    v=dict(vertices=deepcopy(vertices),faces=deepcopy(faces if faces is not None else[[0,1,2,3]]),options=options);v.update(extra);return v


SDK={}
for name,options in [('default',None),('neither',[False,False]),('uv',[True,False]),('colors',[False,True]),('both',[True,True])]:
    SDK['rebuild_mesh_'+name]=mesh(options=options,colors=deepcopy(COLORS),uv=deepcopy(UV),normals=[[1.,0.,0.]]*4,ngons=[dict(vertices=[0,1,2,3],faces=[0])])
SDK['rebuild_mesh_unused']=mesh([[99.,99.,99.],POINTS[0],POINTS[1],[88.,88.,88.],POINTS[2],POINTS[3],[77.,77.,77.]],[[1,2,4,5]],options=[True,True],colors=[[i*20,255-i*20,i*15,i*10]for i in range(7)],uv=[[float(i),float(i*i)]for i in range(7)],normals=[[1.,0.,0.]]*7,ngons=[dict(vertices=[1,2,4,5],faces=[0])])
SDK['rebuild_mesh_warp']=mesh([[0.,0.,0.],[2.,0.,0.],[2.,1.,0.],[0.,1.,.4]],normals=[[1.,0.,0.]]*4)
SDK['rebuild_mesh_seam']=mesh([[0.,0.,0.],[2.,0.,0.],[0.,1.,0.],[2.,0.,0.],[0.,0.,0.],[0.,0.,3.]],[[0,1,2],[3,4,5]],options=[True,True],uv=[[float(i),float(i%3)]for i in range(6)])
SDK['rebuild_mesh_ngon_order']=mesh(POINTS,[[0,1,2],[0,2,3]],options=[True,True],ngons=[dict(vertices=[0,1,2,3],faces=[1,0])])
SDK['rebuild_mesh_tiny']=mesh([[x*1e-8,y*1e-8,z]for x,y,z in POINTS])
SDK['rebuild_mesh_translated']=mesh([[x+1e7+.125,y-1e7+.375,z+1e7+.625]for x,y,z in POINTS])
SDK['rebuild_mesh_reversed']=mesh(POINTS,[[3,2,1,0]])
SDK['rebuild_mesh_opposed']=mesh(POINTS[:3],[[0,1,2],[2,1,0]])
SDK['rebuild_mesh_zero_normals']=mesh(normals=[[0.,0.,0.]]*4)
SDK['rebuild_mesh_collinear']=mesh([[0.,0.,0.],[1.,0.,0.],[2.,0.,0.],[0.,1.,0.]],[[0,1,2],[0,1,3]])


def scene(name='both',arguments='',**extra):
    source=deepcopy(SDK['rebuild_mesh_'+name]);source.pop('options');source['kind']='mesh'
    if 'uv'in source:source['texture_coordinates']=source.pop('uv')
    if 'colors'in source:source['colors']=[c[:3]+[255-c[3]]for c in source['colors']]
    v=dict(command='RebuildMesh',whole=True,preselected=True,sources=[source],mesh_channels=True,command_arguments=arguments);v.update(extra);return v


DISCOVERY={
    'rebuild_mesh_discovery_fresh':scene(),
    'rebuild_mesh_discovery_preselected_flags':scene(arguments='_PreserveTextureCoordinates=_Yes _PreserveVertexColors=_Yes'),
    'rebuild_mesh_discovery_remembered':scene(),
    'rebuild_mesh_discovery_postselect':scene(arguments='_PreserveTextureCoordinates=_No _PreserveVertexColors=_No _All _Enter',whole=False,preselected=False),
    'rebuild_mesh_discovery_cancel':scene(arguments='_PreserveTextureCoordinates=_Yes',whole=False,preselected=False,cancel=True),
    'rebuild_mesh_discovery_after_cancel':scene(),
}

COMMANDS={'rebuild_mesh_command_fresh':scene()}
for name,uv,colors in [('both','Yes','Yes'),('uv','Yes','No'),('colors','No','Yes'),('neither','No','No')]:
    COMMANDS['rebuild_mesh_command_set_'+name]=scene(arguments='_PreserveTextureCoordinates=_'+uv+' _PreserveVertexColors=_'+colors+' _All _Enter',whole=False,preselected=False)
    COMMANDS['rebuild_mesh_command_after_'+name]=scene()
COMMANDS['rebuild_mesh_command_cancel_options']=scene(arguments='_PreserveTextureCoordinates=_Yes _PreserveVertexColors=_Yes _All',whole=False,preselected=False,cancel=True)
COMMANDS['rebuild_mesh_command_after_cancel']=scene()
COMMANDS['rebuild_mesh_command_noop']=scene('tiny')
COMMANDS['rebuild_mesh_command_mixed']=scene()
COMMANDS['rebuild_mesh_command_mixed']['sources'].append(dict(kind='subd',vertices=[[x+5,y,z]for x,y,z in POINTS],faces=[[0,1,2,3]]))
COMMANDS['rebuild_mesh_command_two']=scene()
COMMANDS['rebuild_mesh_command_two']['sources'].append(dict(kind='mesh',vertices=[[x+5,y,z]for x,y,z in POINTS],faces=[[0,1,2,3]],normals=[[1.,0.,0.]]*4))
for name in ['unused','seam','ngon_order','tiny','translated','collinear']:
    COMMANDS['rebuild_mesh_command_'+name]=scene(name)
COMMANDS['rebuild_mesh_command_cancel_empty']=scene(whole=False,preselected=False,cancel=True)

HOLDOUTS={
    'rebuild_mesh_holdout_bare_both':scene(arguments='_PreserveTextureCoordinates _PreserveVertexColors _All _Enter',whole=False,preselected=False),
    'rebuild_mesh_holdout_after_both':scene(),
    'rebuild_mesh_holdout_bare_uv':scene(arguments='_PreserveTextureCoordinates _All _Enter',whole=False,preselected=False),
    'rebuild_mesh_holdout_after_uv':scene(),
    'rebuild_mesh_holdout_cancel_colors':scene(arguments='_PreserveVertexColors _All',whole=False,preselected=False,cancel=True),
    'rebuild_mesh_holdout_after_cancel':scene(),
}


def request(discovery=False,commands=False,holdouts=False):
    if sum([discovery,commands,holdouts])>1:raise ValueError('one mesh rebuilding family required')
    return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=[dict(op='rebuild_mesh_command'if discovery or commands or holdouts else'rebuild_mesh_sdk',id=k,case=k)for k in(HOLDOUTS if holdouts else COMMANDS if commands else DISCOVERY if discovery else SDK)])


def validate(op):
    try:valid=isinstance(op,dict)and set(op)=={'op','id','case'}and op['id']==op['case']and((op['op']=='rebuild_mesh_sdk'and op['case']in SDK)or(op['op']=='rebuild_mesh_command'and(op['case']in DISCOVERY or op['case']in COMMANDS or op['case']in HOLDOUTS)))
    except TypeError:valid=False
    if not valid:raise ValueError('fixed mesh rebuild recipe required')
