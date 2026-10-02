"""Source-only actual edge-separation commands and external Undo/Redo."""
import copy
import json
from .unjoin_edges_cases import request as geometry_request


def request():
    sources={op['id']:op for op in geometry_request()['operations']};ops=[]
    def add(name,breps,components,finish='Enter',**extra):
        ops.append(dict(op='unjoin_edge_command',id=name,sources=[dict(brep=copy.deepcopy(brep)) for brep in breps],components=components,finish=finish,undo_redo=True,**extra))
    for name in ['box-one','box-cap','box-all','box-empty','pair-naked','pair-joined','tube-outer-seam','tube-bottom','nonmanifold-shared']:
        source=sources[name]
        for finish in (['Enter','Cancel'] if name=='box-cap' else ['Enter']):
            add(name+'-'+finish,[source['source']],[[0,e] for e in source['edges']],finish)
    box=sources['box-one']['source']
    add('wrong-face',[box],[[0,0]],kind='face');add('wrong-object',[box],[],object_preselect=True)
    shifted=copy.deepcopy(box); shifted['source']['min'][0]+=10.; shifted['source']['max'][0]+=10.
    for reverse in (False,True):
        add('two-source-reverse-%d'%reverse,[box,shifted],[[1 if reverse else 0,0],[0 if reverse else 1,0]])
    plane=dict(source=dict(type='mesh_brep',vertices=[[0.,0.,0.],[2.,0.,0.],[2.,2.,0.],[0.,2.,0.],[4.,0.,0.],[4.,2.,0.]],faces=[[0,1,2,3],[1,4,5,2]]))
    for finish in ('Enter','Cancel'):
        add('mouse-plane-'+finish,[plane],[[0,2]],finish,pick='mouse')
    strip=copy.deepcopy(plane)
    strip['source']['vertices'].extend([[6.,0.,0.],[6.,2.,0.]])
    strip['source']['faces'].append([4,6,7,5])
    for reverse in (False,True):
        for finish in ('Enter','Cancel'):
            add('mouse-strip-%d-%s'%(reverse,finish),[strip],[[0,e] for e in ([6,2] if reverse else [2,6])],finish,pick='mouse')
    shifted_plane=copy.deepcopy(plane)
    for point in shifted_plane['source']['vertices']: point[0]+=10.
    add('mouse-two-source-reverse',[plane,shifted_plane],[[1,2],[0,2]],pick='mouse')
    orders=[[3,2,1,0],[0,1],[1,2],[0,3],[3,4],[2,3,4],[0,1,2],[0,3,8],[3,0,2,1],[0,1,2,3],[3,0,1,2],list(reversed(range(12)))]
    orders.extend(list(range(n)) for n in range(3,13))
    seen=set()
    for edges in orders:
        name='order-'+'-'.join(map(str,edges))
        if name not in seen: add(name,[box],[[0,e] for e in edges]);seen.add(name)
    split=dict(plane,splits=[[2,[.25,.75]]])
    add('split-pre',[split],[[0,2],[0,7],[0,8]])
    for edges in ([2,7,8],[8,7,2],[2,7],[7,2]):
        add('split-mouse-'+'-'.join(map(str,edges)),[split],[[0,e] for e in edges],pick='mouse')
    for reverse in (False,True):
        add('two-three-%d'%reverse,[box,shifted],[[source,e] for source in ([1,0] if reverse else [0,1]) for e in [0,3,8]])
    return dict(protocol_version=1,iterations=1,operations=ops)


if __name__=='__main__':print(json.dumps(request(),indent=2))
