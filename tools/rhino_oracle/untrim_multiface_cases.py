"""Independent multi-face Untrim sources with optional exact shared topology."""
import copy
import json

from .untrim_component_cases import partial_request, request as component_request


def request():
    base=next(op for op in partial_request()['operations'] if op['id']=='partial-east-reversed')
    band=copy.deepcopy(base['sources'][0]['brep']);band.pop('reversed',None)
    distant=copy.deepcopy(band)
    for point in distant['source']['surface']['control_points']:point['point'][0]+=20.
    wall=copy.deepcopy(band);wall['source'].pop('trim_bounds')
    for point,coordinates in zip(wall['source']['surface']['control_points'],
            [[8.,0.,0.],[8.,10.,0.],[8.,0.,10.],[8.,10.,10.]]):
        point['point']=coordinates
    operations=[]
    for kind,neighbor,edge,joined in [('disconnected',distant,1,False),
            ('joined-picked',wall,1,True),('joined-opposite',wall,3,True),
            ('joined-wall-first',wall,0,True)]:
        for similar in (False,True):
            for keep in (False,True):
                source=dict(source=dict(type='compound',parts=[copy.deepcopy(band),copy.deepcopy(neighbor)]))
                if joined:source['joins']=[[1,4,False]]
                if kind=='joined-wall-first':
                    source['source']['parts'].reverse();source['joins']=[[0,5,False]]
                op=copy.deepcopy(base);op.update(id='multi-%s-similar-%d-keep-%d'%(kind,similar,keep),
                    sources=[dict(brep=source)],components=[[0,edge]],all_similar=similar,
                    keep_trim_objects=keep,view='oblique' if joined else 'top')
                operations.append(op)
    control=copy.deepcopy(base);control['id']='multi-control-single-outer'
    control['sources'][0]['brep'].pop('reversed')
    operations.append(control)
    control=copy.deepcopy(next(op for op in component_request()['operations']
        if op['id']=='tube-hole-similar-0-keep-0-mouse'))
    control['id']='multi-control-joined-hole';operations.append(control)
    return dict(protocol_version=1,iterations=1,operations=operations)


if __name__=='__main__':print(json.dumps(request(),indent=2))
