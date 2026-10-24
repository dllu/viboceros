"""Owned outline command scenes: selection, projection, aggregate coverage and surfaces."""
from copy import deepcopy
try:
    from .mesh_outline_cases import SHAPES, PLANES
except (ImportError, ValueError):
    from mesh_outline_cases import SHAPES, PLANES


def scene(shape='box', **options):
    value = dict(sources=[dict(deepcopy(SHAPES[shape]), kind='mesh')], preselect=True,
                 view='top', plane=deepcopy(PLANES['xy']), frustum=[-.5, .5, -.5, .5, 1., 80.])
    value.update(options)
    return value


COMMANDS = {}
for shape in ['quad', 'box', 'frame', 'overlap', 'disconnected', 'edge_on', 'warped']:
    for preselect in [True, False]:
        name = 'outline_command_' + shape + ('_pre' if preselect else '_post')
        COMMANDS[name] = scene(shape, preselect=preselect)
for view in ['front', 'oblique', 'perspective']:
    COMMANDS['outline_command_box_' + view] = scene(view=view)
    COMMANDS['outline_command_warped_' + view] = scene('warped', view=view, preselect=False)
COMMANDS['outline_command_offset'] = scene(plane=deepcopy(PLANES['offset']))
COMMANDS['outline_command_two_objects'] = scene('quad', sources=[dict(deepcopy(SHAPES['quad']), kind='mesh'),
    dict(kind='mesh', vertices=[[x+2.,y+1.,z+2.] for x,y,z in SHAPES['quad']['vertices']], faces=[[0,1,2,3]])])
COMMANDS['outline_command_two_post'] = dict(deepcopy(COMMANDS['outline_command_two_objects']), preselect=False)
COMMANDS['outline_command_mixed_point'] = scene(sources=[dict(deepcopy(SHAPES['box']), kind='mesh'),dict(kind='point',point=[8.,9.,10.])])
COMMANDS['outline_command_point_only'] = scene(sources=[dict(kind='point',point=[8.,9.,10.])])
COMMANDS['outline_command_cancel'] = scene(preselect=False, cancel=True)
COMMANDS['outline_command_cancel_empty'] = scene(preselect=False, cancel=True, selected=[])
for kind in ['plane', 'brep_box', 'sphere']:
    COMMANDS['outline_command_' + kind] = scene(sources=[dict(kind=kind)])
COMMANDS['outline_command_box_brep_oblique'] = scene(view='oblique', sources=[dict(kind='brep_box')])
COMMANDS['outline_command_surface_and_mesh'] = scene(sources=[dict(kind='plane'),dict(deepcopy(SHAPES['box']),kind='mesh')])


LIFECYCLE = {name.replace('outline_command_', 'outline_lifecycle_'):
             dict(deepcopy(case), view_trace=True) for name, case in COMMANDS.items()}
QUADS = {}
for name, view, preselect in [('pre','perspective',True),('post','perspective',False),
                               ('top','top',True),('oblique','oblique',True)]:
    QUADS['outline_quads_'+name] = scene('warped',view=view,preselect=preselect,view_trace=True,sdk_checks=True)
for name, vertices, face in [
        ('reversed',SHAPES['warped']['vertices'],[3,2,1,0]),
        ('flat',[[x,y,1.]for x,y,z in SHAPES['warped']['vertices']],[0,1,2,3]),
        ('ac',[[0.,0.,1.],[4.,0.,4.],[4.,3.,1.],[0.,3.,1.]],[0,1,2,3])]:
    QUADS['outline_quads_'+name] = scene(view='perspective',view_trace=True,sdk_checks=True,
        sources=[dict(kind='mesh',vertices=deepcopy(vertices),faces=[face])])
ALL_CASES = dict(COMMANDS, **LIFECYCLE)
ALL_CASES.update(QUADS)


def request(lifecycle=False,quads=False):
    return dict(protocol_version=1, iterations=1,
                tolerance=dict(absolute=1e-6, relative=1e-12, angular=1e-6),
                operations=[dict(op='mesh_outline_command', id=name, case=name)
                            for name in (QUADS if quads else LIFECYCLE if lifecycle else COMMANDS)])


def validate(operation):
    try:
        valid = (isinstance(operation, dict) and set(operation) == {'op','id','case'}
                 and operation['op'] == 'mesh_outline_command' and operation['case'] in ALL_CASES
                 and operation['id'] == operation['case'])
    except TypeError:
        valid = False
    if not valid:
        raise ValueError('fixed outline command recipe required')
