"""Independent hole-boundary and unaffected-region MergeFaces holdouts."""
import copy
from .capture_family import capture
from .generate_mesh_merge_faces import request as original


def request():
    query=original();ring=next(op for op in query['operations']if op['id']=='ring-none')
    cases=[]
    hole=copy.deepcopy(ring);hole['ngons']=[dict(vertices=[0,1,2,3,7,11,15,14,13,12,8,4],faces=list(range(8)))];cases.append(('existing-hole',hole))
    mixed=copy.deepcopy(ring);mixed['vertices'] += [[10,0,0],[12,0,0],[12,2,0],[10,2,0]];mixed['faces'] += [[16,17,18],[16,18,19]];mixed['components']=[dict(kind='face',index=i)for i in range(10)];cases.append(('hole-and-pair',mixed))
    partial=copy.deepcopy(ring);partial['components']=[dict(kind='face',index=i)for i in[0,1]];cases.append(('partial-ring',partial))
    edges=copy.deepcopy(ring);edges['components']=[dict(kind='edge',vertices=e)for e in[[1,5],[2,6],[4,5],[6,7],[8,9],[10,11],[9,13],[10,14]]];cases.append(('edge-ring',edges))
    reverse=copy.deepcopy(ring);reverse['faces']=[list(reversed(f))for f in reverse['faces']];cases.append(('reversed-ring',reverse))
    vertex=copy.deepcopy(ring);vertex['components']=[dict(kind='vertex',index=5)];cases.append(('inner-vertex',vertex))
    query['operations']=[]
    for name,op in cases:
        for channels in ['none','all']:
            item=copy.deepcopy(op);item.update(id=name+'-'+channels,channels=channels);query['operations'].append(item)
    return query


if __name__=='__main__':
    capture('mesh_merge_faces_boundaries',request(),['generate_mesh_merge_faces_boundaries.py','generate_mesh_merge_faces.py','mesh_merge_faces_probe.py','hide_pt_input.py','join_probe.py','merge_edges_probe.py','rhino_worker.py','client.py','structural_sdk.py'],
            'VibocerosOracleMergeFaceBoundaries20261029',timeout=300)
