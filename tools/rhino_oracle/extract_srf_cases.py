"""Independent face extraction inputs; native outputs are never source recipes."""
import copy
import json
from .shrink_trimmed_cases import face_request,geometry_request


def request():
    parents=face_request()['operations']
    def source(name):
        return copy.deepcopy(next(op for op in parents if op['id']=='standard-'+name+'-faces-0')['sources'][0])
    joined,disjoint=source('joined'),source('disjoint')
    cases=[('joined-first',[joined],[[0,0]]),('joined-second',[joined],[[0,1]]),
        ('joined-both',[joined],[[0,0],[0,1]]),('joined-reverse-order',[joined],[[0,1],[0,0]]),
        ('disjoint-first',[disjoint],[[0,0]]),('disjoint-both',[disjoint],[[0,0],[0,1]]),
        ('mixed',[joined,disjoint],[[1,1],[0,0]]),('duplicate',[joined],[[0,0],[0,0]]),
        ('mixed-forward',[joined,disjoint],[[0,0],[1,1]]),('mixed-repeat',[joined,disjoint],[[1,1],[0,0]])]
    geometry=geometry_request()['operations']
    def part(name):
        return copy.deepcopy(next(op for op in geometry if op['id']=='standard-'+name+'-pre-1')['sources'][0]['brep'])
    curved=dict(brep=dict(source=dict(type='compound',parts=[part('cylinder-band'),part('signed-cubic')])))
    reversed_curved=copy.deepcopy(curved);reversed_curved['brep']['reversed']=True
    cases.extend([('curved-seam',[curved],[[0,0]]),('signed-rational',[curved],[[0,1]]),
        ('curved-reversed',[reversed_curved],[[0,0],[0,1]])])
    box=dict(brep=dict(source=dict(type='box',min=[0.,0.,0.],max=[2.,3.,4.])))
    cases.extend([('box-all',[box],[[0,f] for f in range(6)]),('box-subset',[box],[[0,0],[0,5],[0,2]])])
    operations=[]
    for name,sources,faces in cases:
        for copying in (False,True):
            for current in (False,True):
                operations.append(dict(op='extract_srf_command',id='%s-copy-%d-current-%d'%(name,copying,current),
                    sources=copy.deepcopy(sources),components=copy.deepcopy(faces),copy=copying,
                    output_current=current,source_layer=True,undo_redo=True))
    return dict(protocol_version=1,iterations=1,operations=operations)


if __name__=='__main__':print(json.dumps(request(),indent=2))
