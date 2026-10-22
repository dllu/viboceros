"""Bounded attributed mesh welding recipes using public mesh geometry."""
from copy import deepcopy
from math import pi

CUBE = [
    [[0,0,0],[0,3,0],[4,3,0],[4,0,0]],
    [[0,0,0],[4,0,0],[4,0,2],[0,0,2]],
    [[4,0,0],[4,3,0],[4,3,2],[4,0,2]],
    [[4,3,0],[0,3,0],[0,3,2],[4,3,2]],
    [[0,3,0],[0,0,0],[0,0,2],[0,3,2]],
    [[0,0,2],[4,0,2],[4,3,2],[0,3,2]],
]


def cube(order=range(6)):
    order=list(order)
    return dict(vertices=[p for i in order for p in CUBE[i]],
                faces=[[4*i+j for j in range(4)] for i in range(len(order))])


PAIR = dict(vertices=[[0,0,0],[4,0,0],[0,3,0],[4,0,0],[0,0,0],[0,-3,0],[99,99,99]],
            faces=[[0,1,2],[3,4,5]])
BENT = deepcopy(PAIR)
BENT['vertices'][5] = [0,0,3]
NONMANIFOLD = deepcopy(PAIR)
NONMANIFOLD['vertices'][6:] = [[0,0,0],[4,0,0],[0,0,3],[99,99,99]]
NONMANIFOLD['faces'].append([6,7,8])
VERTEX_ONLY = dict(vertices=[[0,0,0],[2,0,0],[0,2,0],[0,0,0],[-2,0,0],[0,-2,0]],
                   faces=[[0,1,2],[3,4,5]])
FAN = dict(vertices=[p for i in range(4) for p in
                    [[0,0,0],[[2,0,0],[0,2,0],[-2,0,0],[0,-2,0]][i],
                     [[0,2,0],[-2,0,0],[0,-2,0],[2,0,0]][i]]],
           faces=[[3*i+j for j in range(3)] for i in range(4)])

CASES = {}
for name, source, degrees, channels in [
    ('cube_all',cube(),180,'all'),('cube_plain',cube(),180,'none'),
    ('cube_uv',cube(),180,'uv'),('cube_normals',cube(),180,'normals'),
    ('cube_below',cube(),89,'all'),('cube_right',cube(),90,'all'),
    ('cube_above',cube(),91,'all'),('cube_rotated',cube([3,4,5,0,1,2]),180,'all'),
    ('pair_flat',PAIR,0,'all'),('pair_bent_below',BENT,89,'all'),
    ('pair_bent_right',BENT,90,'all'),('pair_bent_above',BENT,91,'all'),
    ('nonmanifold',NONMANIFOLD,180,'all'),('vertex_only',VERTEX_ONLY,180,'all'),
    ('fan',FAN,0,'all'),
]:
    CASES[name] = dict(source=deepcopy(source),angle=degrees*pi/180.,channels=channels)
reverse = cube()
reverse['vertices'].reverse()
reverse['faces'] = [[23-i for i in f] for f in reverse['faces']]
CASES['cube_reverse_raw'] = dict(source=reverse,angle=pi,channels='all')
triangles = cube()
triangles['faces'] = [t for a,b,c,d in triangles['faces'] for t in [[a,b,c],[a,c,d]]]
CASES['cube_triangles'] = dict(source=triangles,angle=pi,channels='all')
CASES['pair_custom_normals'] = dict(source=deepcopy(PAIR),angle=pi,channels='all',custom_normals=True)


FOLLOWUP = {}
for count in range(2,6):
    for channels in ['all','none']:
        source=cube(range(count))
        # Retain the original invalid face references in this diagnostic family.
        source['faces']=[[4*i+j for j in range(4)] for i in range(6)]
        FOLLOWUP['prefix_'+str(count)+'_'+channels] = dict(source=source,angle=pi,channels=channels)
HALF = dict(vertices=[[0,0,0],[4,0,0],[0,3,0],[0,0,0],[0,-3,0]],faces=[[0,1,2],[1,3,4]])
OTHER_HALF = dict(vertices=[[0,0,0],[4,0,0],[0,3,0],[4,0,0],[0,-3,0]],faces=[[0,1,2],[3,0,4]])
FOLLOWUP['half_shared'] = dict(source=HALF,angle=pi,channels='all')
FOLLOWUP['other_half_shared'] = dict(source=OTHER_HALF,angle=pi,channels='all')
for normal in ['x','opposite']:
    FOLLOWUP['pair_unit_'+normal] = dict(source=deepcopy(PAIR),angle=pi,channels='all',unit_normals=normal)
fan_open = deepcopy(FAN)
fan_open['vertices'] = fan_open['vertices'][:9]
fan_open['faces'] = fan_open['faces'][:3]
FOLLOWUP['open_fan'] = dict(source=fan_open,angle=0.,channels='all')


# Earlier prefix recipes retain invalid face references as diagnostics. Keep
# those producer inputs immutable and capture corrected prefixes separately.
PREFIXES = {}
for count in range(2,7):
    for channels in ['all','none']:
        source=cube(range(count))
        source['faces']=source['faces'][:count]
        PREFIXES['valid_prefix_'+str(count)+'_'+channels]=dict(source=source,angle=pi,channels=channels)


PAIRS = {}
full=cube()
for a,b in [(0,4),(0,1),(1,4),(0,3),(3,4),(0,2),(2,3),(1,2),(1,5),(2,5),(4,5),(3,5)]:
    source=deepcopy(full)
    source['faces']=[source['faces'][a],source['faces'][b]]
    PAIRS['edge_pair_'+str(a)+'_'+str(b)]=dict(source=source,angle=pi,channels='all')
source=deepcopy(PAIR)
source['faces'].reverse()
PAIRS['tri_pair_reverse_faces']=dict(source=source,angle=pi,channels='all')
source=deepcopy(PAIR)
source['vertices'].reverse()
source['faces']=[[6-i for i in f] for f in source['faces']]
PAIRS['tri_pair_reverse_raw']=dict(source=source,angle=pi,channels='all')
source=cube(range(2))
source['faces']=source['faces'][:2][::-1]
PAIRS['quad_pair_reverse_faces']=dict(source=source,angle=pi,channels='all')


def pairs_request():
    result=request()
    result['operations']=[dict(op='mesh_weld_channels',id=name,case=name) for name in PAIRS]
    return result


def prefixes_request():
    result=request()
    result['operations']=[dict(op='mesh_weld_channels_prefixes',id=name,case=name) for name in PREFIXES]
    return result


def followup_request():
    result=request()
    result['operations']=[dict(op='mesh_weld_channels_followup',id=name,case=name) for name in FOLLOWUP]
    return result


def request():
    return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),
                operations=[dict(op='mesh_weld_channels',id=name,case=name) for name in CASES])


def scene(operation):
    if operation.get('op')=='mesh_weld_channels_prefixes':return PREFIXES[operation['case']]
    if operation.get('op')=='mesh_weld_channels_followup':return FOLLOWUP[operation['case']]
    name=operation['case']
    if name in CASES:return CASES[name]
    if name in PAIRS:return PAIRS[name]
    try:
        from .mesh_weld_channels_stress_cases import CASES as stress
    except (ImportError, ValueError):
        from mesh_weld_channels_stress_cases import CASES as stress
    if name in stress:return stress[name]
    try:
        from .mesh_weld_channels_trace_cases import CASES as trace
    except (ImportError, ValueError):
        from mesh_weld_channels_trace_cases import CASES as trace
    if name in trace:return trace[name]
    try:
        from .mesh_weld_channels_options_cases import CASES as options
    except (ImportError, ValueError):
        from mesh_weld_channels_options_cases import CASES as options
    return options[name]


def validate(operation):
    try:
        valid=(isinstance(operation,dict) and set(operation)=={'op','id','case'}
               and operation['op'] in ['mesh_weld_channels','mesh_weld_channels_followup','mesh_weld_channels_prefixes']
               and operation['id']==operation['case'])
        if not valid:raise ValueError('fixed mesh weld channel recipe required')
        scene(operation)
    except (KeyError,TypeError):
        raise ValueError('fixed mesh weld channel recipe required')
