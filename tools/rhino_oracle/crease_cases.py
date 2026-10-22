"""Fixed crease discovery and qualification inputs; no arbitrary native scripts."""
from copy import deepcopy
if __package__:
    from .subdivide_probe import source, QUAD, STRIP, CUBE, CUBE_FACES, SEAM
else:
    from subdivide_probe import source, QUAD, STRIP, CUBE, CUBE_FACES, SEAM

DISCOVERY = {}
for command in ['Crease', 'RemoveCrease']:
    prefix = command.lower() + '_'
    for name, definition in [
        ('subd_edge', source(vertices=STRIP, faces=[[0,1,4,3],[1,2,5,4]], edges=[[1,4]])),
        ('subd_vertex', source(vertices=CUBE, faces=CUBE_FACES, selected_vertices=[0])),
        ('subd_boundary_vertex', source(selected_vertices=[0])),
        ('subd_boundary_edge', source(edges=[[0,1]])),
        ('subd_creased_edge', source(vertices=STRIP, faces=[[0,1,4,3],[1,2,5,4]], creases=[[1,4]], edges=[[1,4]])),
        ('subd_corner', source(corners=[0], selected_vertices=[0])),
        ('subd_cycle', source(vertices=CUBE, faces=CUBE_FACES, edges=[[0,1],[1,2],[2,3],[0,3]])),
        ('mesh_edge', source('mesh', vertices=STRIP, faces=[[0,1,4,3],[1,2,5,4]], edges=[[1,4]])),
        ('mesh_seam', source('mesh', vertices=SEAM, faces=[[0,1,2,3],[4,5,6,7]], edges=[[1,2]])),
        ('mesh_boundary', source('mesh', edges=[[0,1]])),
    ]:
        DISCOVERY[prefix+name] = dict(command=command, sources=[definition])
    for kind in ['subd','mesh']:
        for preselected in [False,True]:
            DISCOVERY[prefix+kind+'_whole_'+('pre'if preselected else'post')] = dict(command=command,sources=[source(kind)],whole=True,preselected=preselected)
    DISCOVERY[prefix+'cancel'] = dict(command=command,sources=[source(vertices=STRIP,faces=[[0,1,4,3],[1,2,5,4]],edges=[[1,4]])],cancel=True)

MAIN = {}
for key, definition in DISCOVERY.items():
    if not definition.get('whole') and not definition.get('cancel'):
        MAIN['main_'+key]=deepcopy(definition)
for command in ['Crease','RemoveCrease']:
    prefix='main_'+command.lower()+'_'
    def add(name,definitions,**options):
        MAIN[prefix+name]=dict(command=command,sources=definitions,**options)
    for indices in [[0,1],[0,2],[0,1,2,3],[0,1,2,3,4,5,6,7]]:
        add('cube_vertices_'+''.join(map(str,indices)),[source(vertices=CUBE,faces=CUBE_FACES,corners=[0,1,2,3],selected_vertices=indices)])
    add('cube_all_edges',[source(vertices=CUBE,faces=CUBE_FACES,creases=[[0,1],[1,2],[2,3],[0,3]],edges=[sorted([f[i],f[(i+1)%len(f)]])for f in CUBE_FACES for i in range(len(f))])])
    for vertex,edge in [(0,[0,1]),(0,[5,6]),(4,[0,1])]:
        add('mixed_'+str(vertex)+'_'+''.join(map(str,edge)),[source(vertices=CUBE,faces=CUBE_FACES,corners=[0],creases=[[0,1]],edges=[edge],selected_vertices=[vertex])])
    add('open_two_vertices',[source(corners=[0,1],selected_vertices=[0,1])])
    add('edge_with_corner',[source(vertices=STRIP,faces=[[0,1,4,3],[1,2,5,4]],corners=[1],edges=[[1,4]])])
    add('multiple_subd',[source(edges=[[0,1]],selected_vertices=[0]),source(vertices=[[x+10,y,z]for x,y,z in STRIP],faces=[[0,1,4,3],[1,2,5,4]],edges=[[1,4]])])
    add('mixed_families',[source(vertices=STRIP,faces=[[0,1,4,3],[1,2,5,4]],edges=[[1,4]]),source('mesh',vertices=[[x+10,y,z]for x,y,z in SEAM],faces=[[0,1,2,3],[4,5,6,7]],edges=[[1,2]])])
    add('mesh_unused',[source('mesh',vertices=STRIP+[[99.,99.,99.]],faces=[[0,1,4,3],[1,2,5,4]],edges=[[1,4]])])
    add('mesh_triangle',[source('mesh',vertices=QUAD,faces=[[0,1,2],[0,2,3]],edges=[[0,2]])])
    add('mesh_nonmanifold',[source('mesh',vertices=[[0.,0.,0.],[4.,0.,0.],[0.,3.,0.],[0.,-3.,0.],[0.,0.,3.]],faces=[[0,1,2],[1,0,3],[0,1,4]],edges=[[0,1]])])
    for kind in ['subd','mesh']:
        add(kind+'_post',[source(kind,vertices=STRIP,faces=[[0,1,4,3],[1,2,5,4]],edges=[[1,4]])],postselect=True)
        add(kind+'_post_cancel',[source(kind,vertices=STRIP,faces=[[0,1,4,3],[1,2,5,4]],edges=[[1,4]])],postselect=True,cancel=True)
    for kind in ['edge','vertex']:
        add('soft_'+kind,[source(vertices=CUBE,faces=CUBE_FACES,edges=[[0,1]],soft=50,selected_vertices=[0]if kind=='vertex'else[])],soft_vertex_only=(kind=='vertex'))

EXTRA = {}
all_edges=[sorted([f[i],f[(i+1)%len(f)]])for f in CUBE_FACES for i in range(len(f))]
grid=[[float(x),float(y),0.2*float(x*y)]for y in [0,2,4]for x in [0,2,4]]
grid_faces=[[0,1,4,3],[1,2,5,4],[3,4,7,6],[4,5,8,7]]
for command in ['Crease','RemoveCrease']:
    prefix='extra_'+command.lower()+'_'
    for name,edges,corners in [('dart',[[0,1]],[]),('crease',[[0,1],[0,3]],[]),('corner_on_chain',[[0,1],[0,3]],[0]),('all_edges',all_edges,[])]:
        for selected in [[0],[0,1]]:
            EXTRA[prefix+name+'_'.join(['']+[str(i)for i in selected])]=dict(command=command,sources=[source(vertices=CUBE,faces=CUBE_FACES,creases=edges,corners=corners,selected_vertices=selected)])
    for corner in [False,True]:
        EXTRA[prefix+'grid_'+str(corner)]=dict(command=command,sources=[source(vertices=grid,faces=grid_faces,creases=[[1,4],[4,7]],corners=[4]if corner else[],selected_vertices=[4])])
    for name,edges,corners in [('smooth',[],[]),('crease',[[0,1],[0,3]],[]),('corner',[[0,1],[0,3]],[0])]:
        EXTRA[prefix+'grip_'+name]=dict(command=command,grips=True,sources=[source(vertices=CUBE,faces=CUBE_FACES,creases=edges,corners=corners,selected_vertices=[0])])
    EXTRA[prefix+'mesh_colors']=dict(command=command,sources=[source('mesh',vertices=SEAM,faces=[[0,1,2,3],[4,5,6,7]],edges=[[1,2]],colors=[[10*i,20*i,30*i,125+i]for i in range(8)])])
    EXTRA[prefix+'mesh_ngon']=dict(command=command,sources=[source('mesh',vertices=STRIP,faces=[[0,1,4,3],[1,2,5,4]],edges=[[1,4]],ngons=[dict(vertices=[0,1,2,5,4,3],faces=[0,1])])])
POLICY = {}
for command in ['Crease','RemoveCrease']:
    prefix='policy_'+command.lower()+'_'
    for selected in [[0],[0,1]]:
        POLICY[prefix+'overlap_'+''.join(map(str,selected))]=dict(command=command,sources=[source(vertices=CUBE,faces=CUBE_FACES,edges=[[0,1]],selected_vertices=selected)])
    POLICY[prefix+'soft_dart']=dict(command=command,soft_vertex_only=True,sources=[source(vertices=CUBE,faces=CUBE_FACES,creases=[[0,1]],edges=[[0,3]],soft=50,selected_vertices=[0])])
    for name,edge in [('boundary',[0,1]),('internal',[1,4])]:
        POLICY[prefix+'ngon_'+name]=dict(command=command,sources=[source('mesh',vertices=STRIP,faces=[[0,1,4,3],[1,2,5,4]],edges=[edge],ngons=[dict(vertices=[0,1,2,5,4,3],faces=[0,1])])])
    POLICY[prefix+'ngon_unrelated']=dict(command=command,sources=[source('mesh',vertices=STRIP+[[x+10,y,z]for x,y,z in STRIP],faces=[[0,1,4,3],[1,2,5,4],[6,7,10,9],[7,8,11,10]],edges=[[1,4]],ngons=[dict(vertices=[0,1,2,5,4,3],faces=[0,1]),dict(vertices=[6,7,8,11,10,9],faces=[2,3])])])
ALL_CASES=dict(DISCOVERY,**MAIN)
ALL_CASES.update(EXTRA)
ALL_CASES.update(POLICY)

MIXED = {}
for command in ['Crease','RemoveCrease']:
    for name,creases,corners,edges in [
        ('smooth_remote',[],[],[[5,6]]),
        ('corner_remote',[[5,6]],[0],[[5,6]]),
        ('crease_remote',[[0,1],[0,3]],[],[[5,6]]),
        ('smooth_both',[],[],[[0,1],[5,6]]),
        ('corner_both',[[5,6]],[0],[[0,1],[5,6]]),
    ]:
        MIXED['mixed_'+command.lower()+'_'+name]=dict(command=command,sources=[source(vertices=CUBE,faces=CUBE_FACES,creases=creases,corners=corners,edges=edges,selected_vertices=[0])])
ALL_CASES.update(MIXED)

OVERLAP = {}
for command in ['Crease','RemoveCrease']:
    OVERLAP['overlap_'+command.lower()+'_valence_four']=dict(command=command,sources=[source(vertices=grid,faces=grid_faces,corners=[4],edges=[[1,4]],selected_vertices=[4])])
    OVERLAP['overlap_'+command.lower()+'_boundary']=dict(command=command,sources=[source(corners=[0],edges=[[0,1]],selected_vertices=[0])])
ALL_CASES.update(OVERLAP)
