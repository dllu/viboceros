"""Fixed command scenes for transverse and coplanar mesh intersections."""
from copy import deepcopy
if __package__:
    from .mesh_boolean_command_cases import scene
else:
    from mesh_boolean_command_cases import scene


def sheet(points):
    return dict(kind='mesh', vertices=points, faces=[[0, 1, 2, 3]])


CASES = {}
for shape in ['overlap', 'disjoint', 'contained', 'identical', 'face_touch',
              'edge_touch', 'point_touch', 'rotated', 'unwelded']:
    for pre in [False, True]:
        CASES[shape+('_pre' if pre else '_post')] = scene('intersect', shape, preselect=pre)
xy = sheet([[0., 0., 0.], [4., 0., 0.], [4., 3., 0.], [0., 3., 0.]])
xz = sheet([[2., -1., -2.], [2., 4., -2.], [2., 4., 2.], [2., -1., 2.]])
coplanar = sheet([[2., 1., 0.], [6., 1., 0.], [6., 4., 0.], [2., 4., 0.]])
for mode in ['Drop', 'Polylines', 'Meshes']:
    for shape, sources in [('crossed', [xy, xz]), ('coplanar', [xy, coplanar])]:
        CASES[shape+'_'+mode.lower()] = dict(operation='intersect', preselect=False,
            sources=deepcopy(sources), arguments='_CoplanarFaces=_'+mode)
for mode in ['Auto', 'Literal', 'Document', 'Zero']:
    CASES['tolerance_'+mode.lower()] = scene('intersect', preselect=False,
        arguments='_Tolerance=_'+mode)
CASES['three_sheets'] = dict(operation='intersect', preselect=False,
    sources=deepcopy([xy, xz, sheet([[1., 2., -1.], [5., 2., -1.], [5., 2., 1.], [1., 2., 1.]])]))
CASES['mixed_point'] = scene('intersect', preselect=True)
CASES['mixed_point']['sources'].append(dict(kind='point', point=[8., 7., 6.]))
CASES['cancel'] = scene('intersect', preselect=False, cancel=True)
CASES = {'mesh_intersect_'+name: value for name, value in CASES.items()}
CONTROLS = {
    'menu': scene('intersect', preselect=False, arguments='_CoplanarFaces _Cancel'),
    'singular_mesh': dict(operation='intersect', preselect=False, sources=deepcopy([xy, coplanar]),
        arguments='_CoplanarFaces=_Mesh'),
    'set_zero': scene('intersect', preselect=False, arguments='_Tolerance=_Zero'),
    'boolean_after_zero': scene('union', preselect=False, command_name='MeshBooleanUnion'),
    'boolean_document': scene('union', preselect=False, command_name='MeshBooleanUnion', arguments='_Tolerance=_Document'),
    'after_boolean_document': scene('intersect', preselect=False),
    'drop_cancel': scene('intersect', preselect=False, arguments='_CoplanarFaces=_Drop', cancel=True),
    'after_drop_cancel': dict(operation='intersect', preselect=False, sources=deepcopy([xy,coplanar])),
}
CONTROLS = {'mesh_intersect_controls_'+name:value for name,value in CONTROLS.items()}
ALL_CASES = dict(CASES, **CONTROLS)
MESH_MODES = {}
for shape in ['identical', 'face_touch', 'edge_touch', 'overlap', 'contained']:
    MESH_MODES['mesh_'+shape] = scene('intersect', shape, preselect=False,
        arguments='_Tolerance=_Auto _CoplanarFaces=_Mesh')
for mode in ['Mesh', 'Drop', 'Polylines']:
    MESH_MODES['sheet_'+mode.lower()] = dict(operation='intersect', preselect=False,
        sources=deepcopy([xy,coplanar]), arguments='_CoplanarFaces=_'+mode)
MESH_MODES['brep_boxes'] = scene('intersect', preselect=False)
for source in MESH_MODES['brep_boxes']['sources']:source['kind']='brep_box'
MESH_MODES = {'mesh_intersect_modes_'+name:value for name,value in MESH_MODES.items()}
ALL_CASES.update(MESH_MODES)

inner = sheet([[1.1, 1.8, 0.], [1.6, 1.8, 0.], [1.6, 2.3, 0.], [1.1, 2.3, 0.]])
ring = dict(kind='mesh', vertices=[
    [0.,0.,0.],[4.,0.,0.],[4.,4.,0.],[0.,4.,0.],
    [1.,1.,0.],[3.,1.,0.],[3.,3.,0.],[1.,3.,0.]],
    faces=[[0,1,5,4],[1,2,6,5],[2,3,7,6],[3,0,4,7]])
wide = sheet([[-1.,-1.,0.],[5.,-1.,0.],[5.,5.,0.],[-1.,5.,0.]])
alternate = deepcopy(xy)
alternate['faces'] = [[0,1,3],[1,2,3]]
islands = dict(kind='mesh', vertices=deepcopy(inner['vertices'])+
    [[p[0]+1.,p[1]-1.,p[2]]for p in inner['vertices']],
    faces=[[0,1,2,3],[4,5,6,7]])
COVERAGE = {}
for name,sources,mode in [
    ('contained',[xy,inner],'Polylines'),
    ('contained_reverse',[inner,xy],'Polylines'),
    ('contained_drop',[xy,inner],'Drop'),
    ('contained_mesh',[xy,inner],'Mesh'),
    ('alternate_diagonals',[xy,alternate],'Polylines'),
    ('holed_patch',[wide,ring],'Polylines'),
    ('disjoint_patches',[xy,islands],'Polylines'),
    ('oblique_contained',[xy,inner],'Polylines'),
]:
    sources = deepcopy(sources)
    if name == 'oblique_contained':
        from math import sqrt
        c = sqrt(.5)
        for source in sources:
            source['vertices'] = [[c*(x-z)+7.,y-5.,c*(x+z)+3.]for x,y,z in source['vertices']]
    COVERAGE['mesh_intersect_coverage_'+name] = dict(operation='intersect',preselect=False,
        sources=sources,arguments='_Tolerance=_Auto _CoplanarFaces=_'+mode)
ALL_CASES.update(COVERAGE)

def triangles(source):
    value = deepcopy(source)
    value['faces'] = [part for face in value['faces'] for part in
        ([face] if len(face)==3 else [face[:3],[face[0],face[2],face[3]]])]
    return value


TOPOLOGY = {}
for name,sources,mode in [
    ('identical_polylines',[xy,xy],'Polylines'),
    ('identical_drop',[xy,xy],'Drop'),
    ('identical_mesh',[xy,xy],'Mesh'),
    ('identical_triangles',[triangles(xy),triangles(xy)],'Polylines'),
    ('holed_triangles_quad',[triangles(wide),ring],'Polylines'),
    ('holed_quad_triangles',[wide,triangles(ring)],'Polylines'),
    ('holed_all_triangles',[triangles(wide),triangles(ring)],'Polylines'),
]:
    TOPOLOGY['mesh_intersect_topology_'+name] = dict(operation='intersect',preselect=False,
        sources=deepcopy(sources),arguments='_Tolerance=_Auto _CoplanarFaces=_'+mode)
ALL_CASES.update(TOPOLOGY)
if __package__:
    from .mesh_intersect_coincident_cases import cases as coincident_cases
    from .mesh_intersect_mixed_cases import cases as mixed_cases
else:
    from mesh_intersect_coincident_cases import cases as coincident_cases
    from mesh_intersect_mixed_cases import cases as mixed_cases
COINCIDENT = coincident_cases(triangles,xy,coplanar)
ALL_CASES.update(COINCIDENT)
MIXED = mixed_cases(sheet,xy,islands)
ALL_CASES.update(MIXED)
FAMILIES = dict(discovery=CASES,controls=CONTROLS,modes=MESH_MODES,
    coverage=COVERAGE,topology=TOPOLOGY,coincident=COINCIDENT,mixed=MIXED)


def request(controls=False, mesh_modes=False, coverage=False, topology=False, family=None):
    if family is None:
        family='topology'if topology else 'coverage'if coverage else 'modes'if mesh_modes else 'controls'if controls else 'discovery'
    if family not in FAMILIES:raise ValueError('fixed MeshIntersect family required')
    return dict(protocol_version=1, iterations=1,
        tolerance=dict(absolute=1e-6, relative=1e-12, angular=1e-6),
        operations=[dict(op='mesh_intersect_command', id=name, case=name) for name in FAMILIES[family]])


def validate(operation):
    if not (isinstance(operation, dict) and set(operation) == {'op', 'id', 'case'}
            and operation.get('op') == 'mesh_intersect_command'
            and isinstance(operation.get('case'), str) and operation['case'] in ALL_CASES
            and operation['id'] == operation['case']):
        raise ValueError('fixed MeshIntersect command recipe required')
