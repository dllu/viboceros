"""Prescribed curved supporting-face constraints and independent partials."""
import copy
if __package__:from .blend_curve_edge_cases import CURVED,SKEW,OTHER,edge
else:from blend_curve_edge_cases import CURVED,SKEW,OTHER,edge

def sources():
    skew=copy.deepcopy(SKEW)
    for j,offset in enumerate([0.,2.,-1.,0.]):
        for i in range(4):skew['control_points'][4*j+i]['point'][1]+=offset
    rational=copy.deepcopy(CURVED)
    for j,w in enumerate([1.,.4,2.,1.]):
        for i in range(2):rational['control_points'][2*j+i]['weight']=w
    curved_edge=copy.deepcopy(skew)
    for i,z in enumerate([0.,.6,-.2,.5]):curved_edge['control_points'][12+i]['point'][2]=z
    weighted=copy.deepcopy(curved_edge)
    for j in range(4):
        for i in range(4):weighted['control_points'][4*j+i]['weight']=[1.,.4,2.,1.][j]*[1.,1.3,.6,1.1][i]
    return dict(cubic=CURVED,skew=SKEW,metric=skew,rational=rational,curved_edge=curved_edge,weighted=weighted)

def discovery(case,pick,line,cubic):
    result={};parents=sources()
    def make(order,source,extra=None,second=None,reverse=False,point=False):
        a=edge(0,[0.,-2.5,.2]if source in[parents['curved_edge'],parents['weighted']]else[0.,-2.5,0.],2)
        b=edge(1,[10.,5.5,0.],0)
        curves=[source,OTHER]
        if point:actions=[a,'Point',[10.,3.,2.]]
        elif second is not None:curves=[source,second];actions=[a,'Curves',pick(1,[10.25,3.,0.])]
        elif reverse:curves=[OTHER,source];actions=[dict(b,curve=0),dict(a,curve=1)]
        else:actions=[a,b]
        return case(['Continuity_1=G'+str(order),'Continuity_2=G'+str(order),'Edges']+actions+(extra or[])+['Enter'],curves=curves,measure_jets=True)
    for name,source in parents.items():
        for order in [3,4]:result['surface_high_'+name+'_g'+str(order)]=make(order,source)
    for order in [3,4]:
        result['surface_high_angle_g'+str(order)]=make(order,parents['metric'],['AtAngle_1=37','AtAngle_2=-23'])
        result['surface_high_reverse_g'+str(order)]=make(order,CURVED,reverse=True)
        result['surface_high_point_g'+str(order)]=make(order,CURVED,point=True)
        result['surface_high_mixed_g'+str(order)]=make(order,parents['metric'],second=cubic)
    return result

def planarity(case):
    if __package__:from .blend_curve_edge_cases import tensor
    else:from blend_curve_edge_cases import tensor
    near=tensor([-5.,-4.,-3.,-2.,-1.,0.],[-5.,0.],lambda i,j:1e-12*[0.,0.,0.,1.,0.,0.][j])
    return {'surface_high_near_plane_g'+str(order):case(['Continuity_1=G'+str(order),'Continuity_2=G'+str(order),'Edges',edge(0,[0.,-2.5,0.],2),edge(1,[10.,5.5,0.],0),dict(click=[{2:3.16,3:2.26,4:1.76}[order],-2.5,0.]),[2000.,-2.5,0.],'Enter'],curves=[near,OTHER],measure_jets=True)for order in [2,3,4]}

def holdouts(case,pick,cubic):
    from math import factorial
    if __package__:from .blend_curve_edge_cases import tensor
    else:from blend_curve_edge_cases import tensor
    parents=sources();result={}
    def boundary(source):
        count=source['control_point_count_u'];row=source['control_points'][-count:];degree=count-1
        weights=[float(factorial(degree)//(factorial(i)*factorial(degree-i)))*p['weight']for i,p in enumerate(row)]
        return [sum(w*p['point'][k]for w,p in zip(weights,row))/sum(weights)for k in range(3)]
    def pair(order,parent,extra=None,second=None):
        second=second or OTHER
        return case(['Continuity_1=G'+str(order),'Continuity_2=G'+str(order),'Edges',edge(0,boundary(parent),2),edge(1,boundary(second)if second is not OTHER else[10.,5.5,0.],2 if second is not OTHER else 0)]+(extra or[])+['Enter'],curves=[parent,second],measure_jets=True)
    chart=copy.deepcopy(parents['weighted']);chart['knots_u']=[-3. if k==0. else 2. for k in chart['knots_u']];chart['knots_v']=[5. if k==0. else 9. for k in chart['knots_v']]
    fourth=tensor([-5.,-4.,-3.,-2.,-1.,0.],[-5.,0.],lambda i,j:[0.,0.,0.,1.,0.,0.][j])
    both=copy.deepcopy(CURVED)
    for p in both['control_points']:
        x,y,z=p['point'];p['point']=[10.-x,3.-y,z+.5]
    def rotate(p):return [.8*p[0]+.6*p[2]+2.,p[1]+1.,-.6*p[0]+.8*p[2]-3.]
    for order in [3,4]:
        suffix='_g'+str(order)
        result['surface_high_chart'+suffix]=pair(order,chart)
        result['surface_high_fourth'+suffix]=pair(order,fourth)
        recipe=copy.deepcopy(pair(order,parents['weighted']));recipe['view']='Perspective'
        for source in recipe['curves']:
            for p in source['control_points']:p['point']=rotate(p['point'])
        for action in recipe['actions']:
            if isinstance(action,dict)and 'pick'in action:action['pick']=rotate(action['pick'])
        result['surface_high_spatial'+suffix]=recipe
        result['surface_high_both'+suffix]=pair(order,parents['rational'],second=both)
        shape=2.26 if order==3 else 1.76
        result['surface_high_shape'+suffix]=pair(order,CURVED,[dict(click=[shape,-2.5,0.]),[3.4,-2.5,0.]])
        result['surface_high_symmetry'+suffix]=pair(order,CURVED,[dict(click=[shape,-2.5,0.],shift=True),[3.4,-2.5,0.]])
        result['surface_high_slide'+suffix]=pair(order,parents['metric'],[dict(click=[0.,-2.5,0.]),[0.,-1.,0.]])
        result['surface_high_join'+suffix]=case(['BlendStart=PickPoint','Continuity_1=G'+str(order),'Continuity_2=G'+str(order),'Edges',edge(0,boundary(parents['metric']),2),'Curves',pick(1,[11.584,3.32,0.]),'Trim=Yes','Join=Yes','Enter'],curves=[parents['metric'],cubic],measure_jets=True)
    cancelled=pair(4,parents['weighted']);cancelled['actions'][-1]='Cancel';cancelled['pending_redo']=True;result['surface_high_cancel']=cancelled
    return result
