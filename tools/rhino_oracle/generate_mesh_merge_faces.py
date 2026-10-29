"""Actual selected face/edge/vertex merges and channel-preserving controls."""
from .capture_family import capture


def request():
    tri=dict(vertices=[[0,0,0],[2,0,0],[2,2,0],[0,2,0]],faces=[[0,1,2],[0,2,3]])
    strip=dict(vertices=[[0,0,0],[2,0,0],[2,2,0],[0,2,0],[4,0,0],[4,2,0]],faces=[[0,1,2,3],[1,4,5,2]])
    bent=dict(vertices=[[0,0,0],[2,0,1],[2,2,0],[0,2,-1],[4,0,2],[4,2,1]],faces=strip['faces'])
    fan=dict(vertices=[[0,0,0],[2,0,0],[0,2,0],[-2,0,0],[0,-2,0]],faces=[[0,1,2],[0,2,3],[0,3,4],[0,4,1]])
    scenes=[('tri',tri,[0,1],[0,2],0),('strip',strip,[0,1],[1,2],1),('bent',bent,[0,1],[1,2],1),('fan',fan,[0,1,2,3],[0,2],0)]
    rows=[]
    for name,scene,faces,edge,vertex in scenes:
        for component,selection in [('faces',[dict(kind='face',index=i)for i in faces]),('edge',[dict(kind='edge',vertices=edge)]),('vertex',[dict(kind='vertex',index=vertex)])]:
            for channels in ['none','all']:
                rows.append(dict(op='mesh_merge_faces',id=name+'-'+component+'-'+channels,components=selection,channels=channels,ngons=[],**scene))
    seam=dict(vertices=strip['vertices']+[[2,0,0],[2,2,0]],faces=[[0,1,2,3],[6,4,5,7]])
    separate=dict(vertices=tri['vertices']+[[10,0,0],[12,0,0],[12,2,0],[10,2,0]],faces=tri['faces']+[[4,5,6],[4,6,7]])
    ring=dict(vertices=[[x,y,0]for y in range(4)for x in range(4)],faces=[[4*y+x,4*y+x+1,4*y+x+5,4*y+x+4]for y in range(3)for x in range(3)if(x,y)!=(1,1)])
    controls=[('single',dict(vertices=tri['vertices'],faces=tri['faces'][:1]),[0],[]),
              ('unwelded',seam,[0,1],[]),('disjoint',separate,[0,1,2,3],[]),('ring',ring,list(range(8)),[]),
              ('ngon',fan,[0,2,3],[dict(vertices=[0,1,2,3],faces=[0,1])]),
              ('empty-selection',strip,[],[])]
    for name,scene,faces,ngons in controls:
        for channels in ['none','all']:
            rows.append(dict(op='mesh_merge_faces',id=name+'-'+channels,components=[dict(kind='face',index=i)for i in faces],channels=channels,ngons=ngons,**scene))
    return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-7,relative=1e-12,angular=1e-10),operations=rows)


if __name__=='__main__':
    capture('mesh_merge_faces',request(),['generate_mesh_merge_faces.py','mesh_merge_faces_probe.py','hide_pt_input.py','join_probe.py','merge_edges_probe.py','rhino_worker.py','client.py','structural_sdk.py'],
            'VibocerosOracleMergeFaces20261029',timeout=450)
