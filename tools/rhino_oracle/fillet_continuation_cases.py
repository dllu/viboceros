"""Nonmeeting curved partners and closed Fillet sources, prescribed before capture."""
from copy import deepcopy
from math import cos,sin,sqrt


def pilot(case,explicit,pick):
    boundary=dict(type='circle',center=[0.,0.,0.],radius=5.,x_axis=[1.,0.,0.],normal=[0.,0.,1.])
    arc=dict(type='arc',points=[[2.,5.,0.],[sqrt(2.),5.+sqrt(2.),0.],[0.,7.,0.]])
    nurbs=dict(type='nurbs',degree=2,control_points=[dict(point=p,weight=1.)for p in [[0.,6.,0.],[.5,7.5,0.],[1.,10.,0.]]],knots=[0.,0.,0.,1.,1.,1.])
    rows={}
    for label,source,picks,styles in [
        ('arc',arc,[pick(0,[3.,4.,0.]),pick(1,[2.*cos(.4),5.+2.*sin(.4),0.])],['ExtendArcsBy=Arc','ExtendOtherCurvesBy=Line']),
        ('tangent_arc',arc,[pick(0,[3.,4.,0.]),pick(1,[2.*cos(.4),5.+2.*sin(.4),0.])],['ExtendArcsBy=Line','ExtendOtherCurvesBy=Line']),
        ('nurbs_line',nurbs,[pick(0,[-2.5,sqrt(18.75),0.]),pick(1,[.15,6.4725,0.])],['ExtendArcsBy=Arc','ExtendOtherCurvesBy=Line']),
        ('nurbs_smooth',nurbs,[pick(0,[-2.5,sqrt(18.75),0.]),pick(1,[.15,6.4725,0.])],['ExtendArcsBy=Arc','ExtendOtherCurvesBy=Smooth'])]:
        for suffix,trim,join in [('parts',True,False),('joined',True,True),('arc_only',False,False)]:
            rows['continuation_'+label+'_'+suffix]=case(explicit(radius=.1,trim=trim,join=join)+styles+picks,curves=[boundary,source])
    for label,end in [('dynamic_accept','Enter'),('dynamic_cancel','Cancel')]:
        r=deepcopy(rows['continuation_tangent_arc_joined']);r['actions'][5]='Dynamic=Yes';r['actions'] += [.15,end]
        if end=='Cancel':r['pending_redo']=True
        rows['continuation_'+label]=r
    return rows


def holdouts(case,explicit,pick):
    base=pilot(case,explicit,pick);rows={}
    for label in ['arc','tangent_arc','nurbs_line','nurbs_smooth']:
        r=deepcopy(base['continuation_'+label+'_joined']);r['actions'][-2:]=list(reversed(r['actions'][-2:]));rows['continuation_'+label+'_reverse_picks']=r
    for label in ['arc','nurbs_smooth']:
        r=deepcopy(base['continuation_'+label+'_joined']);c=r['curves'][1]
        if c['type']=='arc':c['points'].reverse()
        else:c['control_points'].reverse()
        rows['continuation_'+label+'_backward']=r
    for label in ['arc','tangent_arc']:
        r=deepcopy(base['continuation_'+label+'_joined']);r['actions'][1]=.5;rows['continuation_'+label+'_larger_radius']=r
    for suffix in ['parts','joined']:
        r=deepcopy(base['continuation_arc_'+suffix]);r['actions'][1]=.6;r['curves'][1]['points']=[[p[0],p[1]+3.,p[2]]for p in r['curves'][1]['points']];r['actions'][-1]['pick'][1]+=3.;rows['continuation_disjoint_arc_'+suffix]=r
    r=deepcopy(rows['continuation_disjoint_arc_joined']);r['actions'][6]='ExtendArcsBy=Line';rows['continuation_disjoint_tangent_arc']=r
    r=deepcopy(base['continuation_nurbs_smooth_joined']);r['actions'][1]=.6;r['curves'][0]['radius']=3.;r['actions'][-2]['pick']=[-1.5,1.5*sqrt(3.),0.];rows['continuation_disjoint_smooth']=r
    def spatial(p):return [7.+.6*p[0],-2.+p[1],3.+.8*p[0]]
    r=deepcopy(base['continuation_tangent_arc_joined']);r['curves'][0].update(center=spatial([0.,0.,0.]),x_axis=[.6,0.,.8],normal=[-.8,0.,.6]);r['curves'][1]['points']=[spatial(p)for p in r['curves'][1]['points']]
    for a in r['actions'][-2:]:a['pick']=spatial(a['pick'])
    rows['continuation_spatial']=r
    r=deepcopy(rows['continuation_disjoint_arc_joined']);r['actions'][1]=.1;r['pending_redo']=True;rows['continuation_disjoint_too_small']=r
    r=deepcopy(base['continuation_nurbs_smooth_joined']);r['curves'][0]=dict(type='ellipse',center=[0.,0.,0.],radius_x=5.,radius_y=3.,x_axis=[1.,0.,0.],y_axis=[0.,1.,0.]);r['actions'][-2]['pick']=[-2.5,1.5*sqrt(3.),0.];r['pending_redo']=True;rows['continuation_smooth_miss']=r
    return rows


def verification(case,explicit,pick):
    base=pilot(case,explicit,pick);other=holdouts(case,explicit,pick);rows={}
    for radius in [.5,.6,.7,.8,1.]:
        r=deepcopy(other['continuation_disjoint_smooth']);r['actions'][1]=radius;rows['continuation_smooth_bridge_'+str(radius).replace('.','_')]=r
    for scale in [.25,.5,2.]:
        r=deepcopy(base['continuation_nurbs_smooth_joined'] if scale < 1. else other['continuation_disjoint_smooth'])
        r['curves'][1]['control_points']=[dict(point=p,weight=1.)for p in [[0.,6.,0.],[scale/2.,6.+1.5*scale,0.],[scale,6.+3.*scale+scale*scale,0.]]]
        t=scale*.1;r['actions'][-1]['pick']=[t,6.+3.*t+t*t,0.]
        rows['continuation_smooth_span_'+str(scale).replace('.','_')]=r
    r=deepcopy(other['continuation_smooth_miss']);r['curves'][1]['control_points']=[dict(point=p,weight=1.)for p in [[0.,6.,0.],[1.,9.,0.],[2.,16.,0.]]];rows['continuation_smooth_long_miss']=r
    return rows


def branches(case,explicit,pick):
    source=holdouts(case,explicit,pick)['continuation_disjoint_smooth'];rows={}
    for radius in [.6,.7]:
        for label,trim in [('parts',True),('arc_only',False)]:
            r=deepcopy(source);r['actions'][1]=radius;r['actions'][3]='Join=No';r['actions'][4]='Trim='+('Yes'if trim else'No')
            rows['continuation_opposed_'+str(radius).replace('.','_')+'_'+label]=r
    r=deepcopy(source);r['actions'][-2]['pick']=[1.5,1.5*sqrt(3.),0.];rows['continuation_smooth_right_pick']=r
    r=deepcopy(source);r['curves'][1]['control_points'].reverse();rows['continuation_opposed_backward']=r
    return rows
