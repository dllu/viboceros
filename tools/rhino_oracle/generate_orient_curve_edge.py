"""Public curve-to-edge workflows with fixed scenes and prompt-gated input."""
import gzip,hashlib,json
from pathlib import Path
HERE=Path(__file__).resolve().parent

def request():
    xy=[[0,-6,0],[2,-6,0],[2,-5,0],[3,-5,0]]
    corners=[[0,-2,0],[12,-2,0],[12,6,0],[0,6,0]]
    cp=[[0,0,0],[1,0,0],[0,1,0]]
    ops=[]
    def add(name,points=xy,c=corners,plane=cp,end=False,flip=False,reverse=False,reversed_face=False,copy=False,grouped=False,kind='Nurbs',edge=0,finish=None):
        pick=points[-1]if end else points[0]
        aim=[[8,-2,0],[12,2,0],[8,6,0],[0,2,0]][edge]
        target=[[6,-2,0],[12,1,0],[6,6,0],[0,1,0]][edge]
        if c!=corners:aim=[8,-2,2];target=[6,-2,1.5]
        actions=[dict(curve=0,pick=pick),dict(curve=1,edge=edge,pick=aim),'Copy='+('Yes'if copy else'No'),'FlipSurface='+('Yes'if flip else'No'),'ReverseCurve='+('Yes'if reverse else'No'),target]
        prompts=[['Select curve to orient'],['Select target surface edge']]+[['Pick target edge point']]*4
        if copy:actions += [[9,-2,target[2]*1.5],'Cancel'];prompts += [['Pick target edge point']]*2
        if finish is not None:actions=actions[:finish]+['Cancel'];prompts=prompts[:finish]+[prompts[finish]]
        ops.append(dict(op='orient_curve_edge',id=name,kind=kind,grouped=grouped,points=points,corners=c,reversed=reversed_face,cplane=plane,actions=actions,action_prompts=prompts))
    for name,points in [('xy',xy),('xz',[[0,-6,0],[2,-6,0],[2,-6,1],[3,-6,1]]),('yz',[[0,-6,0],[0,-4,0],[0,-4,1],[0,-3,1]])]:
        for end in [False,True]:
            for flip,reverse in [(False,False),(True,False),(False,True),(True,True)]:add(name+'_'+str(int(end))+str(int(flip))+str(int(reverse)),points=points,end=end,flip=flip,reverse=reverse)
    for edge in [1,2,3]:add('boundary_'+str(edge),edge=edge)
    for name,c in [('tilted',[[0,-2,0],[12,-2,3],[12,6,3],[0,6,0]]),('warped',[[0,-2,0],[12,-2,3],[12,6,6],[0,6,0]])]:
        add(name,c=c);add(name+'_reversed',c=c,reversed_face=True);add(name+'_copies',c=c,copy=True,grouped=True)
    add('on_edge_oblique',points=[[2,-6,-1],[1,-6,-2],[1,-4,-1],[1,-2,0]])
    add('copies',copy=True);add('grouped_move',grouped=True);add('grouped_copies',copy=True,grouped=True)
    for finish in [0,1,2]:add('cancel_'+str(finish),finish=finish,grouped=True)
    add('line',kind='Line',points=[[0,-6,0],[3,-6,0]])
    add('polyline',kind='Polyline')
    add('rotated_cplane',plane=[[0,0,0],[0,0,1],[1,0,0]],points=[[0,-6,0],[0,-6,2],[0,-5,2],[0,-5,3]])
    q=dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=ops)
    from .structural_sdk import validate_operations
    validate_operations(q);return q

def option_request():
    import copy
    q=request();base=q['operations'][0];on=next(op for op in q['operations']if op['id']=='on_edge_oblique')
    rows=[]
    for source in [base,on]:
        for flip,reverse in [(True,False),(False,True),(True,True)]:
            op=copy.deepcopy(source);op['id']+='_bare_'+str(int(flip))+str(int(reverse))
            op['actions'][3]='FlipSurface'if flip else'FlipSurface=No'
            op['actions'][4]='ReverseCurve'if reverse else'ReverseCurve=No'
            rows.append(op)
    return dict(q,operations=rows)

def spatial_request():
    import copy
    q=request();base=q['operations'][0];rows=[]
    pairs=[([1,0,0],[1,2,3]),([1,2,3],[1,0,1]),([-1,0,-1],[0,2,1]),([-1,0,-1],[0,0,1]),([0,1,0],[2,-1,3]),([1,0,0],[0,0,1]),([0,0,1],[1,0,0]),([0,0,-1],[1,2,3]),([0,0,1],[0,0,1]),([1,0,0],[0,1,0]),([1,1,1],[-1,-1,2]),([2,-3,1],[-1,2,3])]
    for i,(a,b)in enumerate(pairs):
        for end in [False,True]:
            op=copy.deepcopy(base);op['id']='spatial_'+str(i)+'_'+str(int(end))
            p0=[-3,-10,2];p3=[0,-6,2]
            op['points']=[p0,[p0[j]+a[j]for j in range(3)],[p3[j]-b[j]for j in range(3)],p3]
            op['actions'][0]['pick']=op['points'][-1]if end else op['points'][0]
            rows.append(op)
    for i in [0,1,2,3]:
        op=copy.deepcopy(rows[2*i]);op['id']+='_shape'
        a,b=pairs[i];p0=[-1,-11,5];p3=[0,-6,2]
        op['points']=[p0,[p0[j]+a[j]for j in range(3)],[p3[j]-b[j]for j in range(3)],p3];op['actions'][0]['pick']=p0;rows.append(op)
    for f in [.1,.45,.8]:
        op=copy.deepcopy(rows[0]);op['id']='spatial_pick_'+str(int(f*100));p=op['points']
        op['actions'][0]['pick']=[(1-f)**3*p[0][j]+3*(1-f)**2*f*p[1][j]+3*(1-f)*f*f*p[2][j]+f**3*p[3][j]for j in range(3)];rows.append(op)
    for i in [0,2,4,6]:
        op=copy.deepcopy(rows[i]);op['id']+='_cplane';op['cplane']=[[0,0,0],[0,0,1],[1,0,0]];rows.append(op)
    for scene in ['tilted','warped','warped_reversed','warped_copies']:
        op=copy.deepcopy(next(p for p in q['operations']if p['id']==scene));op['id']='spatial_'+scene
        op['points']=copy.deepcopy(rows[0]['points']);op['actions'][0]['pick']=op['points'][0];rows.append(op)
    return dict(q,operations=rows)

def parallel_request():
    import copy
    q=spatial_request();base=q['operations'][0];rows=[]
    def add(name,a,b,plane=None,end=False,scene=None):
        op=copy.deepcopy(base);op['id']=name;p0=[-3,-10,2];p3=[0,-6,2]
        op['points']=[p0,[p0[j]+a[j]for j in range(3)],[p3[j]-b[j]for j in range(3)],p3]
        op['actions'][0]['pick']=p3 if end else p0
        if plane is not None:op['cplane']=plane
        if scene:
            authored=next(x for x in request()['operations']if x['id']==scene)
            for k in ['corners','reversed']:op[k]=authored[k]
            op['actions'][1:]=authored['actions'][1:];op['action_prompts']=authored['action_prompts']
        rows.append(op)
    for name,a,b in [('both_positive',[0,0,1],[0,0,1]),('both_negative',[0,0,-1],[0,0,-1]),('opposite',[0,0,1],[0,0,-1])]:
        for end in [False,True]:add(name+'_'+str(int(end)),a,b,end=end)
    for name,plane,t in [('rotated_xy',[[0,0,0],[0,1,0],[-1,0,0]],[0,0,1]),('normal_y',[[0,0,0],[0,0,1],[1,0,0]],[0,1,0]),('normal_oblique',[[0,0,0],[1,-1,0],[1,1,-2]],[1,1,1])]:
        for end in [False,True]:add(name+'_'+str(int(end)),t,t,plane=plane,end=end)
    for exponent in [4,6,8,10,12,14]:
        d=10.**-exponent
        add('near_pick_'+str(exponent),[d,0,1],[1,2,3])
        add('near_anchor_'+str(exponent),[1,0,0],[d,0,1])
    for scene in ['tilted','warped_reversed']:add('parallel_'+scene,[0,0,1],[0,0,1],scene=scene)
    return dict(q,operations=rows)

def threshold_request():
    import copy
    q=spatial_request();base=q['operations'][0];rows=[]
    def add(name,a,b,angle=None):
        op=copy.deepcopy(base);op['id']=name;p0=[-3,-10,2];p3=[0,-6,2]
        op['points']=[p0,[p0[j]+a[j]for j in range(3)],[p3[j]-b[j]for j in range(3)],p3];op['actions'][0]['pick']=p0
        if angle is not None:op['angle_tolerance']=angle
        rows.append(op)
    for i,d in enumerate([.1,.02,.015,.01,.001,.0001]):
        add('threshold_pick_'+str(i),[d,0,1],[1,2,3]);add('threshold_anchor_'+str(i),[1,0,0],[d,0,1])
    for i,angle in enumerate([1e-6,.01]):
        add('tolerance_pick_'+str(i),[.0001,0,1],[1,2,3],angle);add('tolerance_anchor_'+str(i),[1,0,0],[.0001,0,1],angle)
    for i,d in enumerate([1e-4,1e-8,1e-12]):add('oblique_anchor_'+str(i),[1,1,1],[-1+d,-1,2])
    return dict(q,operations=rows)

def cutoff_request():
    import copy
    q=spatial_request();base=q['operations'][0];rows=[]
    def add(name,a,b):
        op=copy.deepcopy(base);op['id']=name;p0=[-3,-10,2];p3=[0,-6,2]
        op['points']=[p0,[p0[j]+a[j]for j in range(3)],[p3[j]-b[j]for j in range(3)],p3];op['actions'][0]['pick']=p0;rows.append(op)
    for i,d in enumerate([.11,.14,.15,.3,.34,.49,.5,.8]):
        add('cutoff_pick_'+str(i),[d,0,1],[1,2,3]);add('cutoff_anchor_'+str(i),[1,0,0],[d,0,1])
    for i,d in enumerate([.1,.2]):
        add('cutoff_diagonal_pick_'+str(i),[d,d,1],[1,2,3]);add('cutoff_diagonal_anchor_'+str(i),[1,0,0],[d,d,1])
    return dict(q,operations=rows)

def boundary_request():
    import copy,math
    q=spatial_request();base=q['operations'][0];rows=[]
    for i,c in enumerate([.9899999,.99,.9900001,-.9899999,-.99,-.9900001]):
        t=[math.sqrt(1-c*c),0,c]
        for mode in ['pick','anchor','line']:
            op=copy.deepcopy(base);op['id']='boundary_'+mode+'_'+str(i)
            if mode=='pick':op['points']=[[0,0,0],t,[-1,-8,-1],[0,-6,2]]
            elif mode=='anchor':op['points']=[[-3,-10,2],[-2,-10,2],[-v for v in t],[0,0,0]]
            else:op['points']=[[0,0,0],t];op['kind']='Line'
            op['actions'][0]['pick']=op['points'][0];rows.append(op)
    for i in [0,2,6,10,16,20]:
        op=copy.deepcopy(q['operations'][i]);op['id']+='_polyline';op['kind']='Polyline';rows.append(op)
    return dict(q,operations=rows)

def angle_request():
    import copy,math
    q=spatial_request();base=q['operations'][0];rows=[]
    for i,angle in enumerate([1e-7,.01,.5]):
        for j,c in enumerate([.98,.995]):
            t=[math.sqrt(1-c*c),0,c]
            for mode in ['pick','anchor']:
                op=copy.deepcopy(base);op['id']='angle_'+mode+'_'+str(i)+'_'+str(j);op['angle_tolerance']=angle
                op['points']=[[0,0,0],t,[-1,-8,-1],[0,-6,2]]if mode=='pick'else[[-3,-10,2],[-2,-10,2],[-v for v in t],[0,0,0]]
                op['actions'][0]['pick']=op['points'][0];rows.append(op)
    return dict(q,operations=rows)

def main():
    from .client import OracleClient
    import argparse
    builders={'--options':('orient_curve_edge_options',option_request),'--spatial':('orient_curve_edge_spatial',spatial_request),'--parallel':('orient_curve_edge_parallel',parallel_request),'--threshold':('orient_curve_edge_threshold',threshold_request),'--cutoff':('orient_curve_edge_cutoff',cutoff_request),'--boundary':('orient_curve_edge_boundary',boundary_request),'--angle':('orient_curve_edge_angle',angle_request)}
    parser=argparse.ArgumentParser(description=__doc__);group=parser.add_mutually_exclusive_group()
    for name in builders:group.add_argument(name,dest='recipe',action='store_const',const=name)
    args=parser.parse_args();label,build=builders[args.recipe]if args.recipe else('orient_curve_edge',request)
    q=build();path=HERE/('fixtures/'+label+'.json');path.write_text(json.dumps(q,indent=2)+'\n')
    names=['generate_orient_curve_edge.py','orient_curve_edge_probe.py','client.py','structural_sdk.py','rhino_worker.py','conic_getter_input.py','number_token.py','join_probe.py','merge_edges_probe.py','managed_mouse_callback.py','snap_environment.py','catenary_point_picker.py','group_picking.py']
    digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();sources={n:digest(HERE/n)for n in names}
    print('Capturing',len(q['operations']),'curve-to-edge workflows',flush=True)
    r=OracleClient(settings_scheme='VibocerosOracleOrientCurveEdge20261020').run_rhino(q,timeout=1000)
    assert sources=={n:digest(HERE/n)for n in names};r['provenance']=dict(request_sha256=digest(path),sources=sources)
    (HERE/('observations/'+label+'.json')).write_text(json.dumps(r,indent=2,allow_nan=False)+'\n')
    for name,sha in sources.items():
        p=HERE/'observations/source_snapshots'/(sha+'.gz')
        if not p.exists():p.write_bytes(gzip.compress((HERE/name).read_bytes(),mtime=0))
    print('Recorded curve-to-edge workflows',flush=True)
if __name__=='__main__':main()
