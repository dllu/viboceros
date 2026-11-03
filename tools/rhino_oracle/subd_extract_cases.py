"""Fixed independent editable SubD face extraction recipes."""
from copy import deepcopy


def grid(nx=3,ny=2):
    return dict(points=[[float(4*x),float(3*y),float((x*y)%3)] for y in range(ny+1) for x in range(nx+1)],
                faces=[[y*(nx+1)+x,y*(nx+1)+x+1,(y+1)*(nx+1)+x+1,(y+1)*(nx+1)+x] for y in range(ny) for x in range(nx)])


def cases():
    net=grid();tagged=deepcopy(net);tagged['corners']=[0];tagged['creases']=[[1,5]]
    cube=dict(points=[[0.,0.,0.],[4.,0.,0.],[4.,3.,0.],[0.,3.,0.],[0.,0.,5.],[4.,0.,5.],[4.,3.,5.],[0.,3.,5.]],
              faces=[[0,3,2,1],[4,5,6,7],[0,1,5,4],[1,2,6,5],[2,3,7,6],[3,0,4,7]])
    return {
        'single':dict(sources=[net],faces=[[0,0]]),
        'adjacent':dict(sources=[net],faces=[[0,1],[0,0]]),
        'disjoint':dict(sources=[net],faces=[[0,5],[0,0]]),
        'all':dict(sources=[net],faces=[[0,i] for i in range(6)]),
        'cut':dict(sources=[grid(3,1)],faces=[[0,1]]),
        'tagged':dict(sources=[tagged],faces=[[0,0]]),
        'closed':dict(sources=[cube],faces=[[0,0],[0,2]]),
        'closed_disjoint':dict(sources=[cube],faces=[[0,0],[0,1]]),
        'multiple':dict(sources=[net,cube],faces=[[0,1],[1,2],[0,0],[1,0]],current=True),
    }


def extra_cases():
    net=grid(2,2)
    triangles=dict(points=[[0.,0.,0.],[4.,0.,0.],[4.,3.,1.],[0.,3.,0.],[8.,0.,0.],[8.,3.,2.]],faces=[[0,1,2],[0,2,3],[1,4,5,2]])
    return {
        'default':dict(sources=[grid()],faces=[[0,0]],default_corners=True),
        'point_contact':dict(sources=[net],faces=[[0,0],[0,3]]),
        'interior':dict(sources=[grid(3,3)],faces=[[0,4]]),
        'triangles':dict(sources=[triangles],faces=[[0,0]]),
        'triangle_pair':dict(sources=[triangles],faces=[[0,0],[0,1]]),
    }


def all_cases():
    result=cases();result.update(extra_cases());return result


def validate(op):
    if (not isinstance(op,dict) or set(op)!={'op','id','scene','copy','corners'}
            or op['op']!='subd_extract_command' or not isinstance(op['scene'],str) or op['scene'] not in all_cases()
            or type(op['copy']) is not bool or type(op['corners']) is not bool
            or op['id']!='-'.join((op['scene'],'copy' if op['copy'] else 'remove','corners' if op['corners'] else 'smooth'))):
        raise ValueError('fixed SubD extraction recipe required')
