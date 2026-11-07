"""Single-parent source definitions and ordered picks, independent of outputs."""
from copy import deepcopy

def pilot(case, configured, pick):
    rows={}
    for name,vertices,picks in [
        ('open',[[0.,0.,0.],[4.,0.,0.],[4.,4.,0.]],[[2.5,0.,0.],[4.,1.5,0.]]),
        ('closed',[[0.,0.,0.],[6.,0.,0.],[6.,4.,0.],[0.,4.,0.],[0.,0.,0.]],[[4.5,0.,0.],[6.,1.5,0.]])]:
        for suffix,trim,join in [('parts',True,False),('joined',True,True),('bevel',False,False)]:
            rows['self_'+name+'_'+suffix]=case(configured(join=join,trim=trim)+[pick(0,p) for p in picks],curves=[dict(type='polyline',vertices=vertices)])
    r=deepcopy(rows['self_open_parts']);r['actions']=configured()+[pick(0,[2.5,0.,0.]),'Cancel'];r['pending_redo']=True
    rows['self_cancel_second']=r
    r=deepcopy(rows['self_open_parts']);r['actions']=configured()+[pick(0,[2.,0.,0.]),pick(0,[3.,0.,0.])];r['pending_redo']=True
    rows['self_same_leaf']=r
    return rows

def discovery(case,configured,pick):
    base=pilot(case,configured,pick);rows={}
    for name in ['self_open_parts','self_open_joined','self_closed_parts','self_closed_joined']:
        r=deepcopy(base[name]);r['actions'][-2:]=list(reversed(r['actions'][-2:]));rows[name+'_reverse_picks']=r
    for suffix,join in [('parts',False),('joined',True)]:
        r=deepcopy(base['self_closed_'+suffix]);r['actions']=configured(join=join)+[pick(0,[0.,1.5,0.]),pick(0,[1.5,0.,0.])];rows['self_seam_'+suffix]=r
        for label,distances in [('zero',(0.,0.)),('zero_first',(0.,1.))]:
            r=deepcopy(base['self_open_'+suffix]);r['actions']=configured(distances=distances,join=join)+r['actions'][-2:];r['pending_redo']=True;rows['self_'+label+'_'+suffix]=r
        r=deepcopy(base['self_open_'+suffix]);r['curves'][0]=dict(type='nurbs',degree=1,control_points=[dict(point=p,weight=1.) for p in [[0.,0.,0.],[4.,0.,0.],[4.,4.,0.]]],knots=[0.,0.,1.,2.,2.]);rows['self_kinked_nurbs_'+suffix]=r
        r=deepcopy(base['self_open_'+suffix]);r['curves'][0]=dict(type='polycurve',segments=[dict(type='line',start=[0.,0.,0.],end=[4.,0.,0.]),dict(type='arc',points=[[4.,0.,0.],[3.+2.**-.5,2.**-.5,0.],[3.,1.,0.]])]);r['actions']=configured(distances=(.2,.3),join=join)+[pick(0,[2.5,0.,0.]),pick(0,[3.6,.8,0.])];rows['self_mixed_'+suffix]=r
    return rows

def holdouts(case,configured,pick):
    base=pilot(case,configured,pick);rows={}
    for name in ['self_open_joined','self_closed_joined']:
        r=deepcopy(base[name]);r['curves'][0]['vertices'].reverse();rows[name+'_backward']=r
    r=deepcopy(base['self_open_joined']);r['curves'][0]['vertices']=[[-2.,-1.,0.],[0.,-1.,0.],[0.,0.,0.],[4.,0.,0.],[4.,4.,0.],[6.,4.,0.]];rows['self_interior']=r
    def spatial(p):return [7.+.6*p[0],-2.+p[1],3.+.8*p[0]]
    r=deepcopy(base['self_open_joined']);r['curves'][0]['vertices']=[spatial(p) for p in r['curves'][0]['vertices']]
    for a in r['actions'][-2:]:a['pick']=spatial(a['pick'])
    rows['self_spatial']=r
    r=deepcopy(base['self_open_joined']);r['curves'][0]['vertices']=[[0.,0.,0.],[6.,0.,0.],[6.,4.,0.],[10.,4.,0.],[10.,10.,0.]];r['actions'][-2:]=[pick(0,[4.,0.,0.]),pick(0,[10.,6.,0.])];r['pending_redo']=True;rows['self_nonadjacent']=r
    r=deepcopy(base['self_open_joined']);r['curves'][0]=dict(type='nurbs',degree=2,control_points=[dict(point=p,weight=1.) for p in [[0.,0.,0.],[4.,0.,0.],[4.,4.,0.]]],knots=[0.,0.,0.,1.,1.,1.]);r['actions'][-2:]=[pick(0,[1.75,.25,0.]),pick(0,[3.75,2.25,0.])];r['pending_redo']=True;rows['self_smooth_nurbs']=r
    r=deepcopy(base['self_open_joined']);r['actions']=configured(distances=(9.,1.),join=True)+r['actions'][-2:];r['pending_redo']=True;rows['self_oversized']=r
    r=deepcopy(base['self_open_parts']);r['actions']=configured()+[r['actions'][-2],'Distances',.25,.75,'Join=Yes',r['actions'][-1]];rows['self_options_after_first']=r
    for suffix,join in [('parts',False),('joined',True)]:
        source=dict(type='nurbs',degree=3,control_points=[dict(point=p,weight=1.) for p in [[0.,0.,0.],[5.,0.,0.],[5.,4.,0.],[0.,0.,0.]]],knots=[0.,0.,0.,0.,1.,1.,1.,1.])
        rows['self_nurbs_seam_'+suffix]=case(configured(distances=(.2,.3),join=join)+[pick(0,[1.35,.108,0.]),pick(0,[1.35,.972,0.])],curves=[source],pending_redo=True)
    for suffix,join in [('parts',False),('joined',True)]:
        r=deepcopy(base['self_closed_'+suffix]);r['actions']=configured(distances=(0.,0.),join=join)+r['actions'][-2:];r['pending_redo']=True;rows['self_closed_zero_'+suffix]=r
        for label,vertices,distances,picks in [
            ('flat_span',[[0.,0.,0.],[1.,0.,0.],[4.,0.,0.],[4.,4.,0.]],(3.5,1.),[[2.5,0.,0.],[4.,1.5,0.]]),
            ('kink_span',[[0.,0.,0.],[0.,2.,0.],[4.,2.,0.],[4.,6.,0.]],(4.2,1.),[[2.5,2.,0.],[4.,3.5,0.]])]:
            rows['self_'+label+'_'+suffix]=case(configured(distances=distances,join=join)+[pick(0,p) for p in picks],curves=[dict(type='polyline',vertices=vertices)],pending_redo=True)
    return rows


def spans(case,configured,pick):
    original=holdouts(case,configured,pick);rows={}
    for suffix,join in [('parts',False),('joined',True)]:
        r=deepcopy(original['self_flat_span_'+suffix]);r['actions'][-2]['pick']=[.5,0.,0.];rows['self_flat_far_'+suffix]=r
        r=deepcopy(r);r['actions']=configured(join=join)+r['actions'][-2:];rows['self_flat_far_small_'+suffix]=r
        source=dict(type='polyline',vertices=[[2.,0.,0.],[6.,0.,0.],[6.,4.,0.],[0.,4.,0.],[0.,0.,0.],[2.,0.,0.]])
        rows['self_smooth_seam_'+suffix]=case(configured(join=join)+[pick(0,[1.,0.,0.]),pick(0,[6.,1.5,0.])],curves=[source])
    r=deepcopy(rows['self_smooth_seam_joined']);r['actions'][-2:]=list(reversed(r['actions'][-2:]));rows['self_smooth_seam_reverse_picks']=r
    r=deepcopy(rows['self_smooth_seam_joined']);r['curves'][0]['vertices'].reverse();rows['self_smooth_seam_backward']=r
    return rows
