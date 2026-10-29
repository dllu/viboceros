"""Mixed component priorities and disconnected mesh vertex fans."""
from .capture_family import capture
from .generate_mesh_merge_faces import request as base


def request():
    rows=[]
    fans=dict(vertices=[[0,0,0],[2,0,0],[2,2,0],[0,2,0],[-2,0,0],[-2,-2,0],[0,-2,0]],
              faces=[[0,1,2],[0,2,3],[0,4,5],[0,5,6]])
    seam=dict(vertices=fans['vertices']+[[0,0,0]],faces=[[0,1,2],[0,2,3],[7,4,5],[7,5,6]])
    strip=dict(vertices=[[0,0,0],[2,0,0],[2,2,0],[0,2,0],[4,0,0],[4,2,0],[6,0,0],[6,2,0]],
               faces=[[0,1,2,3],[1,4,5,2],[4,6,7,5]])
    face=lambda *indices:[dict(kind='face',index=i)for i in indices]
    edge=lambda a,b:[dict(kind='edge',vertices=[a,b])]
    vertex=lambda i:[dict(kind='vertex',index=i)]
    cases=[('fan-vertex',fans,vertex(0)),('fan-faces',fans,face(0,1,2,3)),
           ('fan-edge-vertex',fans,edge(0,2)+vertex(0)),('fan-face-vertex',fans,face(0)+vertex(0)),
           ('fan-pair-vertex',fans,face(0,1)+vertex(0)),('fan-edges',fans,edge(0,2)+edge(0,5)),
           ('seam-vertex',seam,vertex(0)),('seam-faces',seam,face(0,1,2,3)),
           ('single-face-far-edge',strip,face(0)+edge(4,5)),
           ('face-between-edges',strip,edge(1,2)+face(1)+edge(4,5)),
           ('faces-overlap-edge',strip,face(0,1)+edge(4,5)),
           ('vertex-overlap-edge',strip,vertex(1)+edge(4,5)),
           ('ignored-edge-vertex',strip,face(0)+edge(1,2)+vertex(4))]
    for name,scene,selection in cases:
        for channels in ['none','all']:
            rows.append(dict(op='mesh_merge_faces',id=name+'-'+channels,components=selection,channels=channels,ngons=[],**scene))
    query=base();query['operations']=rows
    return query


if __name__=='__main__':
    capture('mesh_merge_faces_components',request(),['generate_mesh_merge_faces_components.py','generate_mesh_merge_faces.py',
            'mesh_merge_faces_probe.py','hide_pt_input.py','join_probe.py','merge_edges_probe.py','rhino_worker.py','client.py','structural_sdk.py'],
            'VibocerosOracleMeshMergeComponents20261029',timeout=450)
