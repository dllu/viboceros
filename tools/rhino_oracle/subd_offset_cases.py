"""Independent control nets and bounded SubD offset recipes."""
from copy import deepcopy
if __package__:
    from .subd_extract_cases import cases as extract_cases,extra_cases
    from .subd_join_cases import quad
else:
    from subd_extract_cases import cases as extract_cases,extra_cases
    from subd_join_cases import quad


def scenes():
    q=quad();warped=deepcopy(q);warped['points'][2][2]=3.
    bent=dict(points=[[0.,0.,0.],[4.,0.,0.],[4.,3.,0.],[0.,3.,0.],[7.,0.,4.],[7.,3.,4.]],faces=[[0,1,2,3],[1,4,5,2]])
    creased=deepcopy(bent);creased['creases']=[[1,2]];creased['corners']=[0]
    return dict(quad=q,warped=warped,bent=bent,creased=creased,
                grid=extract_cases()['single']['sources'][0],closed=extract_cases()['closed']['sources'][0],
                triangles=extra_cases()['triangles']['sources'][0],
                disjoint=dict(points=q['points']+quad(10.)['points'],faces=[[0,1,2,3],[4,5,6,7]]))


def validate(op):
    if (not isinstance(op,dict) or set(op)!={'op','id','scene','distance','solid'}
            or op['op']!='subd_offset_sdk' or not isinstance(op['scene'],str) or op['scene'] not in scenes()
            or type(op['solid']) is not bool or type(op['distance']) not in (float,int)
            or op['distance'] not in (-.75,0.,.75)
            or op['id']!='-'.join((op['scene'],str(float(op['distance'])),'solid' if op['solid'] else 'skin'))):
        raise ValueError('fixed SubD offset SDK recipe required')
