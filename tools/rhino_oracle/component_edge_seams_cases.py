"""Fixed SubD cuts, coincident boundary welds and mixed seam batches."""
from copy import deepcopy
try:
    from .subdivide_probe import source,STRIP,SEAM,CUBE,CUBE_FACES
except (ImportError,ValueError):
    from subdivide_probe import source,STRIP,SEAM,CUBE,CUBE_FACES
CYCLE=[[0,1],[1,2],[2,3],[3,0]]
def cube(**values):return source(vertices=CUBE,faces=CUBE_FACES,**values)
def strip(**values):return source(vertices=STRIP,faces=[[0,1,4,3],[1,2,5,4]],**values)
def panel(size,**values):
    return source(vertices=[[float(x),float(y),.2*((x+y)%2)]for y in range(size)for x in range(size)],
                  faces=[[y*size+x,y*size+x+1,(y+1)*size+x+1,(y+1)*size+x]for y in range(size-1)for x in range(size-1)],**values)

CASES={}
SHAPES={
    'strip_smooth':strip(edges=[[1,4]]),
    'strip_crease':strip(creases=[[1,4]],edges=[[1,4]]),
    'strip_corner':strip(creases=[[1,4]],corners=[1],edges=[[1,4]]),
    'cube_smooth_cycle':cube(edges=CYCLE),
    'cube_crease_cycle':cube(creases=CYCLE,edges=CYCLE),
    'cube_open_path':cube(creases=CYCLE,edges=CYCLE[:2]),
    'panel_path':panel(4,edges=[[1,5],[5,9],[9,13]]),
    'panel_cross':panel(3,edges=[[1,4],[3,4],[4,5],[4,7]]),
    'panel_loop':panel(4,edges=[[5,6],[6,10],[10,9],[9,5]]),
    'panel_disjoint':panel(4,edges=[[1,5],[10,14]]),
}
for modify in [True,False]:
    for name,shape in SHAPES.items():
        CASES['edge_unweld_'+name+('_modify'if modify else'_preserve')]=dict(command='UnweldEdge',sources=[deepcopy(shape)],modify_normals=modify)
for name,shape,edges in [
    ('strip_pair',strip(creases=[[1,4]],seed_edges=[[1,4]]),[[1,4]]),
    ('strip_corner_pair',strip(creases=[[1,4]],corners=[1],seed_edges=[[1,4]]),[[1,4]]),
    ('cube_cycle',cube(creases=CYCLE,seed_edges=CYCLE),CYCLE),
    ('cube_one_pair',cube(creases=CYCLE,seed_edges=CYCLE),CYCLE[:1]),
    ('cube_partial_loop',cube(creases=CYCLE,seed_edges=CYCLE),CYCLE[:2]),
]:
    for one in [False,True]:
        definition=deepcopy(shape);definition['edges']=edges
        CASES['edge_weld_'+name+('_one_side'if one else'')]=dict(command='WeldEdge',sources=[definition],seed_unweld=True,edges_by_position=True,one_edge_per_pair=one)
for reverse in [False,True]:
    for weld in [False,True]:
        subd=strip(creases=[[1,4]],edges=[[1,4]])
        mesh=source('mesh',vertices=[[x+10,y,z]for x,y,z in (SEAM if weld else STRIP)],faces=[[0,1,2,3],[4,5,6,7]]if weld else[[0,1,4,3],[1,2,5,4]],edges=[[1,2]]if weld else[[1,4]])
        case=dict(command='WeldEdge'if weld else'UnweldEdge',sources=[subd,mesh])
        if weld:subd['seed_edges']=[[1,4]];case.update(seed_unweld=True,edges_by_position=True)
        else:case['modify_normals']=True
        if reverse:case['sources'].reverse()
        CASES['edge_'+('weld'if weld else'unweld')+'_mixed'+('_reverse'if reverse else'')]=case

# Retain the first family's unsorted-loop diagnostics. New captures explicitly
# normalize every pair before the public component-selection calls.
HOLDOUTS={}
for name,recipe in CASES.items():
    if 'cube_'not in name and 'panel_loop'not in name:continue
    row=deepcopy(recipe)
    for definition in row['sources']:
        for field in ['edges','creases','seed_edges']:
            if field in definition:definition[field]=[sorted(pair)for pair in definition[field]]
    HOLDOUTS['checked_'+name]=row

CONTACTS={}
for name,gap,reverse,one in [('exact',0.,False,False),('one_side',0.,False,True),('cooriented',0.,True,False),('near',1e-7,False,False),('gap',1e-3,False,False)]:
    a=source(vertices=[[0.,0.,0.],[2.,0.,0.],[2.,3.,0.],[0.,3.,0.]],faces=[[0,1,2,3]],edges=[[1,2]])
    b=source(vertices=[[2.+gap,0.,0.],[4.+gap,0.,1.],[4.+gap,3.,1.],[2.+gap,3.,0.]],faces=[[3,2,1,0]]if reverse else[[0,1,2,3]],edges=[]if one else[[0,3]])
    CONTACTS['edge_weld_objects_'+name]=dict(command='WeldEdge',sources=[a,b])

TOLERANCES={}
for name,gap in [('zero',0.),('tiny',1e-10),('epsilon',1e-7),('small',1e-5),('wide',1e-3),('large',.01)]:
    shape=strip(creases=[[1,4]],seed_edges=[[1,4]],selected_edge_indices=[3,7],offset_vertices=[6,7],offset=[gap,0.,0.])
    TOLERANCES['edge_weld_gap_'+name]=dict(command='WeldEdge',sources=[shape],seed_unweld=True)
TOLERANCES['edge_weld_duplicate_side']=dict(command='WeldEdge',sources=[strip(creases=[[1,4]],seed_edges=[[1,4]],selected_edge_indices=[7])],seed_unweld=True)
