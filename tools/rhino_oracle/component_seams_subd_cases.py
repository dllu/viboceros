"""Fixed seam commands on SubD tags, boundaries, soft weights and mixed inputs."""
from copy import deepcopy
try:
    from .subdivide_probe import source,STRIP,CUBE,CUBE_FACES,SEAM
except (ImportError,ValueError):
    from subdivide_probe import source,STRIP,CUBE,CUBE_FACES,SEAM
CASES={}
CYCLE=[[0,1],[1,2],[2,3],[0,3]]
CORNER=[[0,1],[0,3],[0,4]]
def cube(**values):return source(vertices=CUBE,faces=CUBE_FACES,**values)
def strip(**values):return source(vertices=STRIP,faces=[[0,1,4,3],[1,2,5,4]],**values)
EDGES={
    'smooth':[cube(edges=[[0,1]])],
    'hard':[cube(creases=[[0,1]],edges=[[0,1]])],
    'boundary':[source(edges=[[0,1]])],
    'corner':[strip(creases=[[1,4]],corners=[1],edges=[[1,4]])],
    'cycle':[cube(creases=CYCLE,edges=CYCLE)],
    'soft':[cube(edges=[[0,1]],soft=50)],
    'multiple':[cube(creases=[[0,1]],edges=[[0,1]]),
                source(vertices=[[x+10,y,z]for x,y,z in STRIP],faces=[[0,1,4,3],[1,2,5,4]],creases=[[1,4]],edges=[[1,4]])],
    'neighbor_corners':[cube(creases=CORNER+[[1,2],[1,5]],corners=[0,1],edges=[[0,1]])],
}
VERTICES={
    'smooth':[cube(selected_vertices=[0])],
    'dart':[cube(creases=[[0,1]],selected_vertices=[0])],
    'crease':[cube(creases=[[0,1],[0,3]],selected_vertices=[0])],
    'corner':[cube(creases=CORNER,corners=[0],selected_vertices=[0])],
    'boundary':[source(corners=[0],selected_vertices=[0])],
    'soft':[cube(edges=[[0,1]],soft=50,selected_vertices=[0])],
    'overlap':[cube(creases=CORNER+[[1,2],[1,5]],corners=[0,1],selected_vertices=[0,1])],
    'multiple':[cube(creases=CORNER,corners=[0],selected_vertices=[0]),
                source(vertices=[[x+10,y,z]for x,y,z in STRIP],faces=[[0,1,4,3],[1,2,5,4]],corners=[1],selected_vertices=[1])],
}
for command in ['WeldEdge','WeldVertices','UnweldEdge','UnweldVertex']:
    vertex=command in ['WeldVertices','UnweldVertex']
    modes=[True,False]if command.startswith('Unweld')else[None]
    for mode in modes:
        suffix=''if mode is None else '_modify'if mode else '_preserve'
        for name,sources in (VERTICES if vertex else EDGES).items():
            case=dict(command=command,sources=deepcopy(sources))
            if mode is not None:case['modify_normals']=mode
            if vertex and name=='soft':case['soft_vertex_only']=True
            CASES[command.lower()+'_'+name+suffix]=case
    subd=cube(creases=CORNER,corners=[0],selected_vertices=[0])if vertex else strip(creases=[[1,4]],edges=[[1,4]])
    mesh=source('mesh',vertices=[[x+10,y,z]for x,y,z in (STRIP if command.startswith('Unweld')else SEAM)]+[[99,99,99]],
                faces=[[0,1,4,3],[1,2,5,4]]if command.startswith('Unweld')else[[0,1,2,3],[4,5,6,7]],
                selected_vertices=[1]if vertex else [],edges=[]if vertex else[[1,4]]if command.startswith('Unweld')else[[1,2]])
    case=dict(command=command,sources=[subd,mesh])
    if command.startswith('Unweld'):case['modify_normals']=True
    CASES[command.lower()+'_mixed']=case
    if vertex:
        for edit in [False,True]:
            shape=cube(selected_vertices=[0])if command.startswith('Unweld')else cube(creases=CORNER,corners=[0],selected_vertices=[0])
            case=dict(command=command,sources=[shape],grips=True,edit_points=edit)
            if command.startswith('Unweld'):case['modify_normals']=True
            CASES[command.lower()+('_surface_grips'if edit else'_control_grips')]=case


FOLLOWUP={}
for command in ['WeldVertices','UnweldVertex']:
    shapes={
        'boundary_crease':[source(selected_vertices=[0])],
        'boundary_incident_crease':[strip(creases=[[1,4]],corners=[1],selected_vertices=[1])],
        'all':[cube(creases=CORNER,corners=[0],selected_vertices=list(range(8)))],
        'multiple_changed':[cube(creases=CORNER,corners=[0],selected_vertices=[0])if command=='WeldVertices'else cube(selected_vertices=[0]),
                            source(vertices=[[x+10,y,z]for x,y,z in CUBE],faces=CUBE_FACES,
                                   creases=CORNER if command=='WeldVertices'else[],
                                   corners=[0]if command=='WeldVertices'else[],selected_vertices=[0])],
    }
    for name,sources in shapes.items():
        case=dict(command=command,sources=sources)
        if command=='UnweldVertex':case['modify_normals']=True
        FOLLOWUP['followup_'+command.lower()+'_'+name]=case
    for reverse in [False,True]:
        original=deepcopy(CASES[command.lower()+'_mixed'])
        if command=='UnweldVertex':original['sources'][0]=cube(selected_vertices=[0])
        if reverse:original['sources'].reverse()
        FOLLOWUP['followup_'+command.lower()+'_mixed'+('_reverse'if reverse else'')]=original
    for edit in [False,True]:
        case=dict(command=command,sources=[cube(selected_vertices=[0])if command=='WeldVertices'else cube(creases=CORNER,corners=[0],selected_vertices=[0])],grips=True,edit_points=edit)
        if command=='UnweldVertex':case['modify_normals']=True
        FOLLOWUP['followup_'+command.lower()+('_surface_noop'if edit else'_control_noop')]=case

GRIPS={}
for shape,definition in [('cube',cube()),('boundary',source()),('corner',cube(creases=CORNER,corners=[0])),('soft',cube(edges=[[0,1]],soft=50))]:
    for selection in [[0],[0,1]]:
        definition=deepcopy(definition);definition['selected_vertices']=selection
        GRIPS['surface_move_'+shape+('_pair'if len(selection)>1 else'_one')]=dict(command='Move',sources=[definition],grips=True,edit_points=True,command_arguments='0,0,0 0.1,0.2,0.3')
GRIPS['surface_move_cube_all']=dict(command='Move',sources=[cube(selected_vertices=list(range(8)))],grips=True,edit_points=True,command_arguments='0,0,0 0.1,0.2,0.3')
ALL_CASES=dict(CASES,**dict(FOLLOWUP,**GRIPS))


def request(followup=False,grips=False):
    return dict(protocol_version=1,iterations=1,
                tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),
                operations=[dict(op='component_seams_subd',id=name,case=name)for name in (GRIPS if grips else FOLLOWUP if followup else CASES)])


def validate(operation):
    try:
        valid=(isinstance(operation,dict)and set(operation)=={'op','id','case'}
               and operation['op']=='component_seams_subd'and operation['case']in ALL_CASES
               and operation['id']==operation['case'])
    except TypeError:valid=False
    if not valid:raise ValueError('fixed SubD component seam recipe required')
