"""Source-only exterior-chain, interior-hole, and joined-wall Untrim cases."""
import copy
import json
from itertools import permutations
from .untrim_cases import request as surface_request


def _base_request():
    source_cases=surface_request()['operations']
    def source(name):
        return copy.deepcopy(next(op['sources'][0] for op in source_cases if op['id']==name+'-keep-0-pre-0'))
    surface=source('natural');surface.pop('type')
    ops=[]
    def add(name,brep,components,all_similar=False,keep=False,pick='preselect',finish='Cancel'):
        ops.append(dict(op='untrim_command',id=name,sources=[dict(brep=copy.deepcopy(brep))],components=[[0,index] for index in components],
            all_similar=all_similar,keep_trim_objects=keep,pick=pick,finish=finish,undo_redo=True))
    for name,bounds,edge in [('inner',[[2.,8.],[2.,8.]],1),('band',[[2.,8.],[0.,10.]],1),('corner',[[0.,8.],[0.,8.]],1),('half',[[2.,10.],[0.,10.]],3)]:
        brep=dict(source=dict(type='surface_face',surface=surface,trim_bounds=bounds))
        for similar in (False,True):
            for keep in (False,True):add('%s-similar-%d-keep-%d'%(name,similar,keep),brep,[edge],similar,keep)
    add('natural',dict(source=dict(type='surface_face',surface=surface)),[1])
    add('band-natural-edge',dict(source=dict(type='surface_face',surface=surface,trim_bounds=[[2.,8.],[0.,10.]])),[0])
    for name,edge in [('two-holes',1),('paraboloid-annulus',1),('annulus',0)]:
        for similar in (False,True):
            for keep in (False,True):add('%s-similar-%d-keep-%d'%(name,similar,keep),dict(source=source(name)),[edge],similar,keep)
    tube=dict(source=dict(type='solid_tube',radii=[2.,5.],height=8.))
    for similar in (False,True):
        for keep in (False,True):add('tube-hole-similar-%d-keep-%d'%(similar,keep),tube,[3],similar,keep)
    add('box-natural',dict(source=dict(type='box',min=[0.,0.,0.],max=[2.,3.,5.])),[0])
    band=dict(source=dict(type='surface_face',surface=surface,trim_bounds=[[2.,8.],[0.,10.]]))
    add('band-mouse',band,[1],pick='mouse',finish='Enter')
    add('band-multiple-pre',band,[1,3])
    return dict(protocol_version=1,iterations=1,operations=ops)


def request():
    base=_base_request()['operations']
    pre=[copy.deepcopy(op) for op in base if op['pick']=='preselect']
    mouse=[dict(copy.deepcopy(op),id=op['id']+'-mouse',pick='mouse',finish='Enter') for op in base[:35]]
    for op in mouse[30:]:op['view']='oblique'
    return dict(protocol_version=1,iterations=1,operations=pre+mouse)


def partial_request():
    base=next(op for op in _base_request()['operations'] if op['id']=='band-similar-0-keep-0')
    ops=[]
    for label,bounds,edge in [('west',[[2.,8.],[0.,10.]],3),('north',[[0.,10.],[2.,8.]],2),
            ('south',[[0.,10.],[2.,8.]],0),('east-reordered',[[2.,8.],[0.,10.]],2),
            ('east-reversed',[[2.,8.],[0.,10.]],1),('east-shifted',[[2.4,3.6],[-3.,5.]],1)]:
        op=copy.deepcopy(base);op.update(id='partial-'+label,pick='mouse',finish='Enter')
        brep=op['sources'][0]['brep'];brep['source']['trim_bounds']=bounds;op['components']=[[0,edge]]
        if label=='east-reordered':brep['edge_order']=[3,2,1,0]
        if label=='east-reversed':brep['reversed']=True
        if label=='east-shifted':
            brep['source']['surface']['knots_u']=[2.,2.,4.,4.]
            brep['source']['surface']['knots_v']=[-3.,-3.,5.,5.]
        ops.append(op)
    return dict(protocol_version=1,iterations=1,operations=ops)


def history_request():
    bases=request()['operations'];ops=[]
    for kind in ('band','tube-hole'):
        for similar in ((False,True) if kind=='band' else (False,)):
            for keep in (False,True):
                base=next(op for op in bases if op['id']=='%s-similar-%d-keep-%d-mouse'%(kind,similar,keep))
                for repick in (False,True):
                    for finish in ('Enter','Cancel'):
                        op=copy.deepcopy(base);op.update(id='history-%s-similar-%d-keep-%d-repick-%d-%s'%(kind,similar,keep,repick,finish),finish=finish,undo_after=[1])
                        if repick:op['components']*=2
                        ops.append(op)
    return dict(protocol_version=1,iterations=1,operations=ops)


def upper_request():
    ops=[]
    for op in request()['operations']:
        if op['id'].startswith('tube-hole') and op['pick']=='mouse':
            op=copy.deepcopy(op);op['id']=op['id'].replace('tube-hole','tube-upper-hole');op['components']=[[0,5]];ops.append(op)
    return dict(protocol_version=1,iterations=1,operations=ops)


def edge_order_request():
    bases=partial_request()['operations'];ops=[]
    orders=[('source',[0,1,2,3]),('reverse',[3,2,1,0]),('mixed',[1,3,0,2])]
    orders.extend((''.join(map(str,order)),list(order)) for order in permutations(range(4))
        if list(order) not in [item[1] for item in orders[:3]])
    for name,label in [('east-reversed','east'),('west','west'),('north','north'),('south','south')]:
        base=next(op for op in bases if op['id']=='partial-'+name)
        for order_name,order in orders:
            op=copy.deepcopy(base);op['id']='order-'+label+'-'+order_name
            brep=op['sources'][0]['brep'];brep.pop('reversed',None);brep['edge_order']=order
            op['components']=[[0,order.index(base['components'][0][1])]];ops.append(op)
    base=next(op for op in bases if op['id']=='partial-east-reversed')
    for label,splits in [('south',[[0,[4.]]]),('north',[[2,[-6.]]]),
            ('both',[[0,[4.,6.]],[2,[-6.,-4.]]]),('picked',[[1,[3.,7.]]])]:
        op=copy.deepcopy(base);op['id']='order-east-split-'+label
        brep=op['sources'][0]['brep'];brep.pop('reversed',None);brep['splits']=splits;ops.append(op)
    return dict(protocol_version=1,iterations=1,operations=ops)


def curved_partial_request():
    bases=partial_request()['operations'];ops=[]
    source_cases=surface_request()['operations']
    polynomial=copy.deepcopy(next(op for op in source_cases if op['id']=='paraboloid-annulus-keep-0-pre-0')['sources'][0]['surface'])
    rational=copy.deepcopy(bases[0]['sources'][0]['brep']['source']['surface'])
    for index,point in enumerate(rational['control_points']):
        point['weight']=1.+.25*(index%3);point['point'][2]=float(index%2)
    for label,surface,ranges in [('polynomial',polynomial,[-1.,1.]),('rational',rational,[0.,10.])]:
        lo,hi=ranges;inner=[lo+.2*(hi-lo),lo+.8*(hi-lo)]
        for name,side in [('east-reversed',1),('west',3),('north',2),('south',0)]:
            op=copy.deepcopy(next(op for op in bases if op['id']=='partial-'+name))
            op['id']='curved-'+label+'-'+name.replace('-reversed','')
            brep=op['sources'][0]['brep'];brep.pop('reversed',None)
            brep['source']['surface']=copy.deepcopy(surface)
            brep['source']['trim_bounds']=[inner,ranges] if side%2 else [ranges,inner]
            order=[2,0,3,1];brep['edge_order']=order;op['components']=[[0,order.index(side)]]
            ops.append(op)
    return dict(protocol_version=1,iterations=1,operations=ops)


if __name__=='__main__':print(json.dumps(request(),indent=2))
