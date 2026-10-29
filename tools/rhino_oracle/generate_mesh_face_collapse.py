"""Freeze face-collapse geometry, channels and actual command history."""
from .capture_family import capture


def request():
    fan = dict(vertices=[[0,0,0],[2,0,0],[0,2,0],[-2,0,0],[0,-2,0],[2,2,0]],
               faces=[[0,1,2],[0,2,3],[0,3,4],[0,4,1],[1,5,2]])
    strip = dict(vertices=[[0,0,0],[2,0,0],[2,2,0],[0,2,0],[4,0,0],[4,2,0]],
                 faces=[[0,1,2,3],[1,4,5,2]])
    tetra = dict(vertices=[[0,0,0],[2,0,0],[0,2,0],[0,0,3]],
                 faces=[[0,2,1],[0,1,3],[0,3,2],[1,2,3]])
    seam = dict(vertices=fan['vertices']+[[0,0,0],[2,0,0]],
                faces=[[0,1,2],[6,2,3],[6,3,4],[6,4,7],[7,5,2]])
    separate = dict(vertices=fan['vertices']+[[10,0,0],[12,0,0],[10,2,0]],
                    faces=fan['faces']+[[6,7,8]])
    cases = [('fan',fan),('quad',strip),('tetra',tetra),('seams',seam),('disjoint',separate),
             ('single-triangle',dict(vertices=fan['vertices'][:3],faces=[[0,1,2]])),
             ('single-quad',dict(vertices=strip['vertices'][:4],faces=[[0,1,2,3]])),
             ('nonplanar',dict(vertices=[[0,0,0],[2,0,1],[2,2,0],[0,2,-2],[4,0,0],[4,2,0]],faces=strip['faces']))]
    return dict(protocol_version=1,iterations=1,
                tolerance=dict(absolute=1e-7,relative=1e-12,angular=1e-10),
                operations=[dict(op='mesh_face_collapse',id=name+'-'+channels,face=0,channels=channels,**scene)
                            for name,scene in cases for channels in ['none','all']])


if __name__ == '__main__':
    capture('mesh_face_collapse',request(),[
        'generate_mesh_face_collapse.py','mesh_face_collapse_probe.py','join_probe.py',
        'merge_edges_probe.py','rhino_worker.py','client.py','structural_sdk.py'],
        'VibocerosOracleFaceCollapse20261029',timeout=450)
