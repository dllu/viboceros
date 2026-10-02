"""Independent sources for both shrink commands; no native outputs as inputs."""
import copy
import json
from .untrim_cases import request as untrim_request
from .untrim_component_cases import partial_request


def request():
    parents=untrim_request()['operations']
    def source(name):
        return dict(source=copy.deepcopy(next(op for op in parents if op['id']==name+'-keep-0-pre-0')['sources'][0]))
    rectangular=copy.deepcopy(partial_request()['operations'][0]['sources'][0]['brep'])
    rectangular.pop('reversed',None)
    compound=dict(source=dict(type='compound',parts=[copy.deepcopy(rectangular),source('annulus')]))
    cases=[(name,[source(name)]) for name in ('outer','hole','annulus','full-border','natural',
           'shifted-domains','paraboloid-disk','paraboloid-annulus','cylinder','triangle','kinky','reversed-hole')]
    cases.extend([('rectangular',[rectangular]),('compound',[compound]),
                  ('multiple',[source('natural'),rectangular,source('annulus')])])
    operations=[]
    for edge in (False,True):
        for name,sources in cases:
            for pre in (False,True):
                operations.append(dict(op='shrink_trimmed_srf_to_edge_command' if edge else 'shrink_trimmed_srf_command',
                    id='%s-%s-pre-%d'%('edge' if edge else 'standard',name,pre),
                    sources=[dict(brep=copy.deepcopy(s)) for s in sources],preselect=pre,
                    order=list(reversed(range(len(sources)))),undo_redo=False))
    return dict(protocol_version=1,iterations=1,operations=operations)


def extended_request():
    import itertools
    import math
    base=request();operations=[]
    for edge in (False,True):
        parent=next(op for op in base['operations'] if op['id']==('edge' if edge else 'standard')+'-multiple-pre-0')
        for pre in (False,True):
            for order in itertools.permutations(range(3)):
                op=copy.deepcopy(parent);op.update(id='%s-order-%s-pre-%d'%('edge' if edge else 'standard',''.join(map(str,order)),pre),preselect=pre,order=list(order),undo_redo=True)
                operations.append(op)
        parent=next(op for op in base['operations'] if op['id']==('edge' if edge else 'standard')+'-annulus-pre-0')
        for pre in (False,True):
            op=copy.deepcopy(parent);op.update(id='%s-rotated-circle-pre-%d'%('edge' if edge else 'standard',pre),preselect=pre,undo_redo=True)
            source=op['sources'][0]['brep']['source']
            for boundary in source['boundaries']:
                for key in ('curve','parameter_curve'):
                    for cp in boundary[key]['control_points']:
                        x,y,z=cp['point'];cp['point']=[5.+(x-y)/math.sqrt(2.),5.+(x+y-10.)/math.sqrt(2.),z]
            x,y=source['interior_uv'];source['interior_uv']=[5.+(x-y)/math.sqrt(2.),5.+(x+y-10.)/math.sqrt(2.)]
            operations.append(op)
    return dict(protocol_version=1,iterations=1,operations=operations)


def geometry_request():
    import math
    from .untrim_multiface_cases import request as multiface_request
    base=request();outer=copy.deepcopy(base['operations'][0]['sources'][0]['brep'])
    def loop_source(points,degree,knots,weights=None,interior=(5.,3.)):
        result=copy.deepcopy(outer);source=result['source'];source['interior_uv']=list(interior)
        curve=dict(degree=degree,knots=knots,control_points=[dict(point=list(p),weight=1. if weights is None else weights[i]) for i,p in enumerate(points)])
        source['boundaries']=[dict(curve=curve,parameter_curve=copy.deepcopy(curve))]
        return result
    cubic=loop_source([[2.,2.,0.],[8.,2.,0.],[8.,8.,0.],[2.,2.,0.]],3,[0.]*4+[1.]*4)
    triangle=loop_source([[1.,1.,0.],[9.,1.,0.],[6.,8.,0.],[1.,1.,0.]],1,[0.,0.,1.,2.,3.,3.],interior=(5.,3.))
    triangle['splits']=[[0,[1.,2.]]]
    ellipse=copy.deepcopy(next(op for op in base['operations'] if op['id']=='standard-annulus-pre-0')['sources'][0]['brep'])
    for boundary in ellipse['source']['boundaries']:
        for key in ('curve','parameter_curve'):
            for cp in boundary[key]['control_points']:
                x,y,z=cp['point'];x=(x-5.)*.75;y=(y-5.)*.5
                cp['point']=[5.+x*math.cos(.37)-y*math.sin(.37),5.+x*math.sin(.37)+y*math.cos(.37),z]
    ellipse['source']['interior_uv']=[3.,5.]
    cylinder=copy.deepcopy(next(op for op in base['operations'] if op['id']=='standard-cylinder-pre-0')['sources'][0]['brep']['source'])
    cylinder.pop('type')
    band=dict(source=dict(type='surface_face',surface=cylinder,trim_bounds=[[0.,4.],[1.,4.]]))
    partial=copy.deepcopy(band);partial['source']['trim_bounds'][0]=[.375,3.625]
    joined=copy.deepcopy(next(op for op in multiface_request()['operations'] if op['id']=='multi-joined-picked-similar-0-keep-0')['sources'][0]['brep'])
    shifted=copy.deepcopy(outer)
    for axis,delta in [('u',1e12),('v',-2e12)]:
        shifted['source']['surface']['knots_'+axis]=[x+delta for x in shifted['source']['surface']['knots_'+axis]]
    for boundary in shifted['source']['boundaries']:
        for cp in boundary['parameter_curve']['control_points']:
            cp['point'][0]+=1e12;cp['point'][1]-=2e12
    shifted['source']['interior_uv']=[1e12+2.,-2e12+2.]
    cases=[('cubic',cubic),('mixed-iso-triangle',triangle),('rotated-ellipse',ellipse),('cylinder-band',band),('cylinder-partial',partial),('joined',joined),('large-uv-origin',shifted),('sphere',dict(source=dict(type='sphere',radius=3.)))]
    operations=[]
    for edge in (False,True):
        for name,source in cases:
            for pre in (False,True):
                operations.append(dict(op='shrink_trimmed_srf_to_edge_command' if edge else 'shrink_trimmed_srf_command',
                    id='%s-%s-pre-%d'%('edge' if edge else 'standard',name,pre),sources=[dict(brep=copy.deepcopy(source))],preselect=pre,order=[0],undo_redo=True))
    # Identical source recipes and pick order with fresh owned UUIDs establish
    # whether renewal ordering is stable across native command runs.
    for pre in (False,True):
        parent=next(op for op in base['operations'] if op['id']=='standard-multiple-pre-%d'%pre)
        for trial in range(4):
            op=copy.deepcopy(parent);op['id']='repeat-order-pre-%d-trial-%d'%(pre,trial);operations.append(op)
    signed=loop_source([[2.,2.,0.],[2.,8.,0.],[8.,8.,0.],[2.,2.,0.]],3,[0.]*4+[1.]*4,
        weights=[1.,-.1,1.,1.],interior=(4.5,4.))
    for edge in (False,True):
        for pre in (False,True):
            operations.append(dict(op='shrink_trimmed_srf_to_edge_command' if edge else 'shrink_trimmed_srf_command',
                id='%s-signed-cubic-pre-%d'%('edge' if edge else 'standard',pre),sources=[dict(brep=copy.deepcopy(signed))],preselect=pre,order=[0],undo_redo=True))
    return dict(protocol_version=1,iterations=1,operations=operations)


def face_request():
    from .untrim_multiface_cases import request as multiface_request
    parents=request()['operations']+geometry_request()['operations']
    def source(name):
        return copy.deepcopy(next(op for op in parents if op['id']=='standard-'+name+'-pre-1')['sources'][0])
    disjoint=dict(brep=copy.deepcopy(next(op for op in multiface_request()['operations']
        if op['id']=='multi-disconnected-similar-0-keep-0')['sources'][0]['brep']))
    operations=[]
    for edge in (False,True):
        prefix='edge' if edge else 'standard'
        op_name='shrink_trimmed_srf_to_edge_command' if edge else 'shrink_trimmed_srf_command'
        for name,src,faces in [('annulus',source('annulus'),[0]),('joined',source('joined'),[0]),
                ('joined',source('joined'),[1]),('joined',source('joined'),[0,1]),
                ('disjoint',disjoint,[0]),('disjoint',disjoint,[1]),('disjoint',disjoint,[0,1]),
                ('natural',source('natural'),[0]),('reversed-hole',source('reversed-hole'),[0])]:
            operations.append(dict(op=op_name,id=prefix+'-'+name+'-faces-'+''.join(map(str,faces)),
                sources=[copy.deepcopy(src)],preselect=True,order=[0],undo_redo=True,
                components=[[0,i] for i in faces]))
        for faces in ([1],[1,1,0]):
            operations.append(dict(op=op_name,id=prefix+'-mixed-whole-faces-'+''.join(map(str,faces)),
                sources=[source('annulus'),copy.deepcopy(disjoint)],preselect=True,order=[0,1],undo_redo=True,
                components=[[1,i] for i in faces],objects=[0]))
    return dict(protocol_version=1,iterations=1,operations=operations)


def face_picking_request():
    import copy
    source=next(op for op in face_request()['operations'] if op['id']=='standard-disjoint-faces-0')['sources'][0]
    disjoint=copy.deepcopy(source)
    operations=[]
    distant=copy.deepcopy(disjoint['brep']['source']['parts'][1])
    for cp in distant['source']['surface']['control_points']:cp['point'][0]+=20.
    for name,steps,cancel in [('first',[(0,0,'sub')],False),('second',[(0,1,'sub')],False),
            ('both',[(0,0,'sub'),(0,1,'sub')],False),('toggle',[(0,0,'sub'),(0,0,'sub'),(0,1,'sub')],False),
            ('cancel',[(0,0,'sub')],True),('mixed',[(1,0,'plain'),(0,1,'sub')],False)]:
        operations.append(dict(op='shrink_trimmed_srf_command',id='standard-face-clicks-'+name,
            sources=[copy.deepcopy(disjoint),dict(brep=copy.deepcopy(distant))],preselect=False,order=[0,1],undo_redo=True,
            components=[],pick='sequence',finish='Cancel' if cancel else 'Enter',
            steps=[dict(kind='click',component=[s,f],modifiers=m) for s,f,m in steps]))
    return dict(protocol_version=1,iterations=1,operations=operations)


def picking_control_request():
    """Same sources and input driver, with one unmodified whole-object click."""
    result=face_picking_request();result['operations']=result['operations'][:1]
    operation=result['operations'][0]
    operation['id']='standard-plain-click-control'
    operation['steps'][0]['modifiers']='plain'
    operation['undo_redo']=False
    return result


if __name__=='__main__':print(json.dumps(request(),indent=2))
