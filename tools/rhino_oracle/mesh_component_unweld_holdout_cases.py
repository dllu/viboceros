"""Independent closed-fan, selection-order and non-manifold command holdouts."""
from copy import deepcopy
try:
    from .mesh_component_unweld_cases import CUBE,BENT
except (ImportError, ValueError):
    from mesh_component_unweld_cases import CUBE,BENT
CASES={}


def add(name,source,kind,indices):
    for modify in [False,True]:
        case=dict(source=deepcopy(source),kind=kind,indices=indices,channels='all',
                  modify_normals=modify,prime_options=True,route='command')
        case['stored_normals']=[[[1.,0.,0.],[0.,1.,0.],[0.,0.,1.]][i%3]for i in range(len(source['vertices']))]
        case['transparency']=[(17*i)%256 for i in range(len(source['vertices']))]
        CASES['holdout_'+name+'_'+('modify'if modify else 'preserve')]=case


for name,indices in [('corner',[0,1,2]),('all',list(range(12))),('disjoint',[0,11]),
                     ('reverse',[4,1,0]),('pair',[0,2])]:
    add('cube_edges_'+name,CUBE,'edge',indices)
rotated=deepcopy(CUBE);rotated['faces']=rotated['faces'][2:]+rotated['faces'][:2]
add('cube_rotated',rotated,'edge',[0,2,4])
four=dict(vertices=[[0,0,0],[4,0,0],[0,2,0],[0,0,3],[0,-2,0],[0,0,-3],[99,99,99]],
          faces=[[0,1,2],[1,0,3],[0,1,4],[1,0,5]])
add('four_face_edge',four,'edge',[0]);add('four_face_vertex',four,'vertex',[0])
octa=dict(vertices=[[0,0,3],[2,0,0],[0,4,0],[-3,0,0],[0,-2,0],[0,0,-4],[99,99,99]],
          faces=[[0,1,2],[0,2,3],[0,3,4],[0,4,1],[5,2,1],[5,3,2],[5,4,3],[5,1,4]])
add('octa_edge',octa,'edge',[0]);add('octa_vertex',octa,'vertex',[0])
tilted=deepcopy(BENT)
tilted['vertices']=[[x+2*y-z,2*x-y+z,x+y+2*z]for x,y,z in tilted['vertices']]
add('tilted_edge',tilted,'edge',[0]);add('tilted_vertex',tilted,'vertex',[0])


def request():
    return dict(protocol_version=1,iterations=1,
                tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),
                operations=[dict(op='mesh_component_unweld_commands',id=name,case=name) for name in CASES])
