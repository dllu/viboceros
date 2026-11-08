"""Distinct-open zero-radius Fillet inputs, with actual Undo/Redo observation."""
from copy import deepcopy
from math import sqrt


def pilot(case, explicit, pick):
    rows = {}
    for label, trim, join in [('parts',True,False),('joined',True,True),('untrimmed',False,False)]:
        rows['open_zero_meeting_'+label] = case(
            explicit(radius=0.,trim=trim,join=join)+[pick(0,[3.7,0.,0.]),pick(1,[4.,.3,0.])],
            capture_undo=True,pending_redo=True)
    for label, trim in [('parts',True),('untrimmed',False)]:
        rows['open_zero_parallel_'+label] = case(
            explicit(radius=0.,trim=trim)+[pick(0,[3.7,0.,0.]),pick(1,[3.7,2.,0.])],
            curves=[dict(type='line',start=[0.,0.,0.],end=[4.,0.,0.]),dict(type='line',start=[0.,2.,0.],end=[4.,2.,0.])],
            capture_undo=True,pending_redo=True)
    rows['open_zero_extension_parts'] = case(
        explicit(radius=0.)+[pick(0,[2.7,0.,0.]),pick(1,[4.,1.3,0.])],
        curves=[dict(type='line',start=[0.,0.,0.],end=[3.,0.,0.]),dict(type='line',start=[4.,1.,0.],end=[4.,4.,0.])],
        capture_undo=True,pending_redo=True)
    for label, tail in [('cancel',['Cancel']),('accept',['Enter'])]:
        r=deepcopy(rows['open_zero_meeting_parts']);r['actions'][5]='Dynamic=Yes';r['actions']+=tail
        rows['open_zero_dynamic_'+label]=r
    for label,start,end in [('to_positive',0.,.5),('from_positive',.5,0.)]:
        r=deepcopy(rows['open_zero_meeting_joined']);r['actions'][1]=start;r['actions'][5]='Dynamic=Yes';r['actions'] += [end,'Enter']
        rows['open_zero_'+label]=r
    return rows


def holdouts(case, explicit, pick):
    base=pilot(case,explicit,pick);rows={}
    for suffix in ['parts','joined']:
        r=deepcopy(base['open_zero_meeting_'+suffix]);r['actions'][-2:]=list(reversed(r['actions'][-2:]))
        rows['open_zero_reverse_'+suffix]=r
    horizontal=dict(type='line',start=[-4.,0.,0.],end=[4.,0.,0.])
    vertical=dict(type='line',start=[0.,-4.,0.],end=[0.,4.,0.])
    for suffix,join in [('parts',False),('joined',True)]:
        rows['open_zero_crossing_'+suffix]=case(explicit(radius=0.,join=join)+[pick(0,[2.,0.,0.]),pick(1,[0.,2.,0.])],curves=[horizontal,vertical],capture_undo=True,pending_redo=True)
    r=deepcopy(rows['open_zero_crossing_joined']);r['actions'][-2:]=list(reversed(r['actions'][-2:]));rows['open_zero_crossing_reverse']=r
    arc=dict(type='arc',points=[[1.,0.,0.],[sqrt(.5),sqrt(.5),0.],[0.,1.,0.]])
    nurbs=dict(type='nurbs',degree=2,control_points=[dict(point=p,weight=1.)for p in [[0.,0.,0.],[1.,0.,0.],[2.,1.,0.]]],knots=[0.,0.,0.,1.,1.,1.])
    rows['open_zero_meeting_arc']=case(explicit(radius=0.)+[pick(0,[.3,sqrt(.91),0.]),pick(1,[0.,2.5,0.])],curves=[arc,dict(type='line',start=[0.,1.,0.],end=[0.,4.,0.])],capture_undo=True,pending_redo=True)
    rows['open_zero_meeting_nurbs']=case(explicit(radius=0.)+[pick(0,[1.5,.5625,0.]),pick(1,[2.,2.,0.])],curves=[nurbs,dict(type='line',start=[2.,1.,0.],end=[2.,5.,0.])],capture_undo=True,pending_redo=True)
    for style in ['Arc','Line']:
        rows['open_zero_arc_'+style.lower()]=case(explicit(radius=0.)+['ExtendArcsBy='+style,'ExtendOtherCurvesBy=Line',pick(0,[.3,sqrt(.91),0.]),pick(1,[-.5,2.5,0.])],curves=[arc,dict(type='line',start=[-.5,2.,0.],end=[-.5,3.,0.])],capture_undo=True,pending_redo=True)
    for style in ['Line','Smooth']:
        rows['open_zero_nurbs_'+style.lower()]=case(explicit(radius=0.)+['ExtendArcsBy=Arc','ExtendOtherCurvesBy='+style,pick(0,[1.5,.5625,0.]),pick(1,[3.,3.,0.])],curves=[nurbs,dict(type='line',start=[3.,2.,0.],end=[3.,4.,0.])],capture_undo=True,pending_redo=True)
    def spatial(p):return [7.+.6*p[0],-2.+p[1],3.+.8*p[0]]
    r=deepcopy(rows['open_zero_crossing_parts'])
    for c in r['curves']:
        for k in ['start','end']:c[k]=spatial(c[k])
    for a in r['actions'][-2:]:a['pick']=spatial(a['pick'])
    rows['open_zero_spatial']=r
    r=deepcopy(rows['open_zero_crossing_parts']);r['curves'][1].update(start=[0.,-4.,-2.],end=[0.,4.,2.]);r['actions'][-1]['pick']=[0.,2.,1.];rows['open_zero_noncoplanar']=r
    for suffix,trim in [('parts',True),('untrimmed',False)]:
        r=deepcopy(rows['open_zero_crossing_parts']);r['curves'][1].update(start=[0.,-4.,.5],end=[0.,4.,.5]);r['actions'][4]='Trim='+('Yes'if trim else'No');r['actions'][-1]['pick']=[0.,2.,.5]
        rows['open_zero_skew_'+suffix]=r
    r=deepcopy(base['open_zero_meeting_parts']);r['curves'][1].update(start=[4.,0.,0.],end=[8.,0.,0.]);r['actions'][-1]['pick']=[4.3,0.,0.];rows['open_zero_collinear']=r
    return rows


def overlaps(case, explicit, pick):
    base=holdouts(case,explicit,pick)['open_zero_collinear'];rows={}
    for label,a,b in [('short',2.,2.),('unequal',4.,6.),('long',100.,100.),('separated',3.,4.)]:
        r=deepcopy(base);start=a if label!='separated'else 4.
        r['curves'][0]['end']=[a,0.,0.];r['curves'][1].update(start=[start,0.,0.],end=[start+b,0.,0.])
        r['actions'][-2:]=[pick(0,[a-.3,0.,0.]),pick(1,[start+.3,0.,0.])]
        rows['open_zero_collinear_'+label]=r
    for label,indices in [('first',[0]),('second',[1]),('both',[0,1])]:
        r=deepcopy(base)
        for i in indices:r['curves'][i]['start'],r['curves'][i]['end']=r['curves'][i]['end'],r['curves'][i]['start']
        rows['open_zero_collinear_reverse_'+label]=r
    r=deepcopy(base);r['actions'][-2:]=list(reversed(r['actions'][-2:]));rows['open_zero_collinear_reverse_picks']=r
    r=deepcopy(base)
    def spatial(p):return [10.+.6*p[0],-2.+p[1],3.+.8*p[0]]
    for c in r['curves']:
        for k in ['start','end']:c[k]=spatial(c[k])
    for a in r['actions'][-2:]:a['pick']=spatial(a['pick'])
    rows['open_zero_collinear_spatial']=r
    r=deepcopy(base);r['curves'][0]['end']=[6.,0.,0.];r['actions'][-1]=pick(1,[6.3,0.,0.]);rows['open_zero_overlap']=r
    r=deepcopy(base);r['actions'][3]='Join=Yes';rows['open_zero_collinear_joined']=r
    return rows


def bounds(case, explicit, pick):
    base=holdouts(case,explicit,pick)['open_zero_collinear'];rows={}
    for label in ['parts','unequal','reversed','joined']:
        r=deepcopy(base);r['actions'].insert(6,'ExtendOtherCurvesBy=Smooth')
        if label=='unequal':r['curves'][1]['end']=[10.,0.,0.]
        if label=='reversed':
            for c in r['curves']:c['start'],c['end']=c['end'],c['start']
        if label=='joined':r['actions'][3]='Join=Yes'
        rows['open_zero_smooth_collinear_'+label]=r
    for label,style,end_y in [('line','Line',2.4),('smooth','Smooth',2.4),('beyond','Line',2.000004)]:
        rows['open_zero_far_'+label]=case(explicit(radius=0.)+['ExtendOtherCurvesBy='+style,pick(0,[.3,0.,0.]),pick(1,[.3,2.+.3*(end_y-2.)/4.,0.])],
            curves=[dict(type='line',start=[0.,0.,0.],end=[4.,0.,0.]),dict(type='line',start=[0.,2.,0.],end=[4.,end_y,0.])],capture_undo=True,pending_redo=True)
    return rows


def orientations(case, explicit, pick):
    base=holdouts(case,explicit,pick)['open_zero_collinear'];rows={}
    for label,u,origin in [('translated',[1.,0.,0.],[100.,20.,3.]),('vertical',[0.,1.,0.],[0.,0.,0.]),('negative',[-1.,0.,0.],[0.,0.,0.]),('tilted',[-.6,0.,.8],[10.,-2.,3.]),('dominant',[-.8,0.,.6],[10.,-2.,3.])]:
        r=deepcopy(base)
        def transform(x):return [a+x*b for a,b in zip(origin,u)]
        for c in r['curves']:
            for k in ['start','end']:c[k]=transform(c[k][0])
        r['actions'][-2:]=[pick(0,transform(3.)),pick(1,transform(5.))]
        r['actions'].insert(6,'ExtendOtherCurvesBy=Smooth')
        rows['open_zero_smooth_orientation_'+label]=r
    return rows
