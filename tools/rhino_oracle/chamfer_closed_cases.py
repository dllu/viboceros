"""Distinct closed-source picks authored independently of Chamfer outputs."""
from copy import deepcopy
from math import sqrt

def pilot(case,configured,pick):
    circle=dict(type='circle',center=[0.,0.,0.],radius=5.,x_axis=[1.,0.,0.],normal=[0.,0.,1.])
    line=dict(type='line',start=[0.,-8.,0.],end=[0.,8.,0.])
    rows={}
    for name,curves,picks in [
        ('circle_line',[circle,line],[pick(0,[2.5,sqrt(18.75),0.]),pick(1,[0.,6.8,0.])]),
        ('two_circles',[circle,dict(circle,center=[6.,0.,0.])],[pick(0,[1.5,sqrt(22.75),0.]),pick(1,[4.5,sqrt(22.75),0.])]),
        ('rectangle_line',[dict(type='polyline',vertices=[[-5.,-5.,0.],[5.,-5.,0.],[5.,5.,0.],[-5.,5.,0.],[-5.,-5.,0.]]),line],[pick(0,[2.5,5.,0.]),pick(1,[0.,6.8,0.])])]:
        for suffix,trim,join in [('bevel',False,False),('parts',True,False),('joined',True,True)]:
            rows['closed_'+name+'_'+suffix]=case(configured(trim=trim,join=join)+picks,curves=curves)
    r=deepcopy(rows['closed_circle_line_parts']);r['actions']=configured()+[r['actions'][-2],'Cancel'];r['pending_redo']=True;rows['closed_cancel_second']=r
    return rows

def holdouts(case,configured,pick):
    original=pilot(case,configured,pick);rows={}
    for name in ['closed_circle_line_joined','closed_two_circles_joined','closed_rectangle_line_parts']:
        r=deepcopy(original[name]);r['actions'][-2:]=list(reversed(r['actions'][-2:]));rows[name+'_reverse_picks']=r
    r=deepcopy(original['closed_circle_line_parts']);r['actions'][-2:]=[pick(0,[2.5,-sqrt(18.75),0.]),pick(1,[0.,-6.8,0.])];rows['closed_circle_line_lower']=r
    r=deepcopy(original['closed_circle_line_parts']);r['actions'][-1]=pick(1,[0.,2.,0.]);rows['closed_circle_line_inner']=r
    r=deepcopy(original['closed_circle_line_parts']);points=[[5.,0.,0.],[5.,5.,0.],[0.,5.,0.],[-5.,5.,0.],[-5.,0.,0.],[-5.,-5.,0.],[0.,-5.,0.],[5.,-5.,0.],[5.,0.,0.]]
    r['curves'][0]=dict(type='nurbs',degree=2,control_points=[dict(point=p,weight=1. if i%2==0 else sqrt(.5)) for i,p in enumerate(points)],knots=[0.,0.,0.,1.,1.,2.,2.,3.,3.,4.,4.,4.]);rows['closed_nurbs_circle_line']=r
    r=deepcopy(original['closed_circle_line_parts']);r['curves'][0]=dict(type='ellipse',center=[0.,0.,0.],x_axis=[1.,0.,0.],y_axis=[0.,1.,0.],radius_x=5.,radius_y=3.);r['actions'][-2]=pick(0,[3.,2.4,0.]);rows['closed_ellipse_line']=r
    r=deepcopy(original['closed_circle_line_joined'])
    def spatial(p):return [20.+.6*p[0],-8.+p[1],3.+.8*p[0]]
    r['curves'][0].update(center=spatial([0.,0.,0.]),x_axis=[.6,0.,.8],normal=[-.8,0.,.6]);r['curves'][1]=dict(type='line',start=spatial([0.,-8.,0.]),end=spatial([0.,8.,0.]));r['actions'][-2:]=[pick(0,spatial([2.5,sqrt(18.75),0.])),pick(1,spatial([0.,6.8,0.]))];r['view']='Perspective';rows['closed_spatial']=r
    r=deepcopy(original['closed_circle_line_joined']);r['curves'][1]=dict(type='line',start=[0.,6.,0.],end=[0.,8.,0.]);rows['closed_line_extension']=r
    r=deepcopy(original['closed_two_circles_parts']);r['curves'][1]['center']=[12.,0.,0.];r['actions'][-1]=pick(1,[7.5,sqrt(4.75),0.]);r['pending_redo']=True;rows['closed_separated']=r
    for name in ['closed_circle_line_parts','closed_circle_line_joined','closed_two_circles_joined']:
        r=deepcopy(original[name]);r['actions']=configured(distances=(0.,0.),join=name.endswith('joined'))+r['actions'][-2:];r['pending_redo']=True;rows[name+'_zero']=r
    return rows


def final(case,configured,pick):
    rows=holdouts(case,configured,pick)
    r=rows.pop('closed_spatial');r['view']='Top';rows['closed_spatial_top']=r
    return rows


def branches(case,configured,pick):
    base=pilot(case,configured,pick);rows={}
    for suffix,join in [('parts',False),('joined',True)]:
        r=deepcopy(base['closed_two_circles_'+suffix]);r['curves'][1]['center']=[10.,0.,0.];r['actions']=configured(join=join)+[pick(0,[sqrt(22.75),1.5,0.]),pick(1,[10.-sqrt(22.75),1.5,0.])];r['pending_redo']=True;rows['closed_tangent_circles_'+suffix]=r
        r=deepcopy(base['closed_circle_line_'+suffix]);r['curves'][1]=dict(type='nurbs',degree=2,control_points=[dict(point=p,weight=1.) for p in [[-1.5,-8.,0.],[0.,-8.,0.],[1.5,8.,0.]]],knots=[0.,0.,0.,1.,1.,1.]);r['actions']=configured(join=join)+[r['actions'][-2],pick(1,[1.35,6.44,0.])];rows['closed_nonlinear_nurbs_'+suffix]=r
    r=deepcopy(base['closed_two_circles_parts']);r['curves'][1]['center']=[0.,0.,0.];r['actions'][-1]=pick(1,[-2.5,sqrt(18.75),0.]);r['actions']=configured()+[dict(curve=0),dict(curve=1)];r['source_seed_points']=[[5.,0.,0.],[5.,0.,0.]];r['pending_redo']=True;rows['closed_coincident_circles']=r
    r=deepcopy(base['closed_circle_line_parts']);r['curves'][1]=dict(type='line',start=[5.,-8.,0.],end=[5.,8.,0.]);r['actions'][-2:]=[pick(0,[sqrt(18.75),2.5,0.]),pick(1,[5.,3.,0.])];r['pending_redo']=True;rows['closed_tangent_line']=r
    r=deepcopy(base['closed_circle_line_parts']);r['curves'][1]=dict(type='line',start=[6.,-8.,0.],end=[6.,8.,0.]);r['actions'][-1]=pick(1,[6.,3.,0.]);r['pending_redo']=True;rows['closed_disjoint_line']=r
    r=deepcopy(base['closed_circle_line_bevel']);r['actions']=configured(distances=(0.,0.),trim=False)+r['actions'][-2:];r['pending_redo']=True;rows['closed_zero_untrimmed']=r
    r=deepcopy(base['closed_circle_line_joined']);r['actions']=configured(distances=(.5,4.),join=True)+r['actions'][-2:];r['pending_redo']=True;rows['closed_consumed_line']=r
    r=deepcopy(base['closed_circle_line_parts']);r['curves'][0]['normal']=[0.,0.,-1.];rows['closed_circle_backward']=r
    return rows
