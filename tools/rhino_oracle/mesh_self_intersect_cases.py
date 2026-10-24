"""Fixed self-intersection scenes, including ordinary mesh adjacency controls."""
from copy import deepcopy


def mesh(vertices, faces):
    return dict(kind='mesh', vertices=vertices, faces=faces)


def combine(*parts):
    vertices, faces = [], []
    for part in parts:
        offset = len(vertices)
        vertices.extend(deepcopy(part['vertices']))
        faces.extend([[i+offset for i in f] for f in part['faces']])
    return mesh(vertices, faces)


xy = mesh([[0.,0.,0.],[4.,0.,0.],[4.,3.,0.],[0.,3.,0.]], [[0,1,2,3]])
wall = mesh([[2.,-1.,-2.],[2.,4.,-2.],[2.,4.,2.],[2.,-1.,2.]], [[0,1,2,3]])
overlap = mesh([[2.,1.,0.],[6.,1.,0.],[6.,4.,0.],[2.,4.,0.]], [[0,1,2,3]])
box = mesh([[x,y,z] for z in [0.,2.] for y in [0.,3.] for x in [0.,4.]],
    [[0,2,3,1],[4,5,7,6],[0,1,5,4],[2,6,7,3],[0,4,6,2],[1,3,7,5]])
adjacent = mesh([[0.,0.,0.],[2.,0.,0.],[0.,2.,0.],[0.,0.,2.]], [[0,1,2],[1,0,3]])
folded = mesh([[0.,0.,0.],[2.,0.,0.],[0.,2.,0.],[1.,1.,0.]], [[0,1,2],[1,0,3]])
shared_vertex = mesh([[0.,0.,0.],[4.,0.,0.],[0.,4.,0.],[2.,2.,-1.],[2.,2.,1.]],
    [[0,1,2],[0,3,4]])
CASES = {}
for name, sources in [
    ('flat',[xy]), ('box',[box]), ('adjacent',[adjacent]),
    ('crossing',[combine(xy,wall)]), ('coplanar',[combine(xy,overlap)]),
    ('identical_faces',[combine(xy,xy)]), ('folded',[folded]),
    ('shared_vertex',[shared_vertex]), ('separate_objects',[xy,wall]),
    ('two_self_meshes',[combine(xy,wall),combine(xy,wall)]),
    ('mixed_point',[combine(xy,wall),dict(kind='point',point=[8.,7.,6.])]),
    ('brep',[dict(box,kind='brep_box')]),
]:
    for pre in [False,True]:
        CASES['mesh_self_intersect_'+name+('_pre'if pre else '_post')] = dict(
            operation='intersect',preselect=pre,sources=deepcopy(sources))
CASES['mesh_self_intersect_cancel'] = dict(operation='intersect',preselect=False,
    sources=[combine(xy,wall)],cancel=True)
CASES['mesh_self_intersect_tolerance_option'] = dict(operation='intersect',preselect=False,
    sources=[combine(xy,wall)],arguments='_Tolerance=_Document')


CONTROLS = {}
for name, sources, args, extra in [
    ('set_zero',[combine(xy,wall)],'_Tolerance=_Zero',{}),
    ('intersect_after_zero',[xy,wall],'',dict(command_name='MeshIntersect')),
    ('boolean_document',[box,box],'_Tolerance=_Document',dict(command_name='MeshBooleanUnion',operation='union')),
    ('after_boolean_document',[combine(xy,wall)],'',{}),
    ('drop_cancel',[combine(xy,overlap)],'_CoplanarFaces=_Drop',dict(cancel=True)),
    ('after_drop_cancel',[combine(xy,overlap)],'',{}),
    ('mesh_mode',[combine(xy,overlap)],'_CoplanarFaces=_Mesh',{}),
    ('polylines_mode',[combine(xy,overlap)],'_CoplanarFaces=_Polylines',{}),
    ('menu',[combine(xy,wall)],'_CoplanarFaces _Cancel',{}),
    ('flat_chain',[mesh([[0.,0.,0.],[1.,0.,0.],[0.,1.,0.],[1.,1.,0.],[2.,0.,0.]],
        [[0,1,2],[1,3,2],[1,4,3]])],'',{}),
    ('edge_contact',[combine(xy,mesh([[1.,3.,0.],[3.,3.,0.],[3.,4.,0.],[1.,4.,0.]],[[0,1,2,3]]))],'',{}),
    ('three_sheets',[combine(xy,wall,mesh([[1.,2.,-1.],[5.,2.,-1.],[5.,2.,1.],[1.,2.,1.]],[[0,1,2,3]]))],'',{}),
    ('identical_boxes',[combine(box,box)],'',{}),
]:
    value=dict(operation='intersect',preselect=False,sources=deepcopy(sources),arguments=args)
    value.update(extra)
    CONTROLS['mesh_self_intersect_controls_'+name]=value
FAMILIES=dict(discovery=CASES,controls=CONTROLS)
ALL_CASES=dict(CASES,**CONTROLS)


def request(family='discovery'):
    return dict(protocol_version=1,iterations=1,
        tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),
        operations=[dict(op='mesh_self_intersect_command',id=name,case=name)for name in FAMILIES[family]])


def validate(operation):
    if not (isinstance(operation,dict) and set(operation)=={'op','id','case'}
            and operation.get('op')=='mesh_self_intersect_command'
            and isinstance(operation.get('case'),str) and operation['case'] in ALL_CASES and operation['id']==operation['case']):
        raise ValueError('fixed MeshSelfIntersect command recipe required')
