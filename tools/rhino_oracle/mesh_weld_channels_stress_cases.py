"""Fixed permutation and fan holdouts for independently inferred Weld behavior."""
from copy import deepcopy
try:
    from .mesh_weld_channels_cases import cube
except (ImportError, ValueError):
    from mesh_weld_channels_cases import cube

CASES={}

def add(name,source,angle=3.141592653589793):
    CASES[name]=dict(source=deepcopy(source),angle=angle,channels='all')

def permuted(source,stride,offset):
    count=len(source['vertices'])
    order=[(i*stride+offset)%count for i in range(count)]
    assert len(set(order))==count
    remap={old:new for new,old in enumerate(order)}
    return dict(vertices=[source['vertices'][i] for i in order],
                faces=[[remap[i] for i in f] for f in source['faces']])

for rotation in [1,2,4,5]:
    add('holdout_cube_rotation_'+str(rotation),cube(list(range(rotation,6))+list(range(rotation))))
add('holdout_cube_reverse_faces',cube(list(reversed(range(6)))))
for stride,offset in [(5,0),(5,3),(7,1),(23,0)]:
    add('holdout_cube_raw_'+str(stride)+'_'+str(offset),permuted(cube(),stride,offset))
for count in [2,3,4,5]:
    add('holdout_cube_prefix_raw_'+str(count),permuted(cube(range(count)),4*count-1,0))

RINGS={
    3:[[3,0],[0,3],[-3,-3]],
    4:[[3,0],[0,3],[-3,0],[0,-3]],
    5:[[3,0],[1,3],[-3,1],[-1,-3],[2,-2]],
    6:[[3,0],[3,3],[0,3],[-3,0],[-3,-3],[0,-3]],
    7:[[3,0],[3,3],[0,3],[-3,3],[-3,0],[-3,-3],[0,-3]],
    8:[[3,0],[3,3],[0,3],[-3,3],[-3,0],[-3,-3],[0,-3],[3,-3]],
}
for count,ring in sorted(RINGS.items()):
    for closed in [True,False]:
        n=count if closed else count-1
        source=dict(vertices=[p for i in range(n) for p in
                              [[0,0,0],ring[i]+[0],ring[(i+1)%count]+[0]]],
                    faces=[[3*i+j for j in range(3)] for i in range(n)])
        add('holdout_fan_'+str(count)+'_'+('closed' if closed else 'open'),source,0.)

for n in [2,3]:
    source=dict(vertices=[p for x in range(n) for y in range(2) for p in
                          [[x,y,0],[x+1,y,0],[x+1,y+1,0],[x,y+1,0]]],
                faces=[[4*i+j for j in range(4)] for i in range(2*n)])
    add('holdout_grid_quads_'+str(n),source,0.)
    triangles=deepcopy(source)
    triangles['faces']=[t for a,b,c,d in source['faces'] for t in [[a,b,c],[a,c,d]]]
    add('holdout_grid_triangles_'+str(n),triangles,0.)


def request():
    return dict(protocol_version=1,iterations=1,
                tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),
                operations=[dict(op='mesh_weld_channels',id=name,case=name) for name in CASES])
