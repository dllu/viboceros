"""Nonmeeting curved sources and a closed boundary, prescribed before capture."""
from copy import deepcopy
from math import sin,cos,sqrt

def pilot(case,configured,pick):
    boundary=dict(type='circle',center=[0.,0.,0.],radius=5.,x_axis=[1.,0.,0.],normal=[0.,0.,1.])
    arc=dict(type='arc',points=[[2.,5.,0.],[sqrt(2.),5.+sqrt(2.),0.],[0.,7.,0.]])
    nurbs=dict(type='nurbs',degree=2,control_points=[dict(point=p,weight=1.) for p in [[0.,6.,0.],[.5,7.5,0.],[1.,10.,0.]]],knots=[0.,0.,0.,1.,1.,1.])
    rows={}
    for label,source,picks,option in [
        ('arc',[boundary,arc],[pick(0,[3.,4.,0.]),pick(1,[2.*cos(.4),5.+2.*sin(.4),0.])],'ExtendArcsBy=Arc'),
        ('tangent_arc',[boundary,arc],[pick(0,[3.,4.,0.]),pick(1,[2.*cos(.4),5.+2.*sin(.4),0.])],'ExtendArcsBy=Line'),
        ('nurbs_line',[boundary,nurbs],[pick(0,[-2.5,sqrt(18.75),0.]),pick(1,[.15,6.4725,0.])],'ExtendOtherCurvesBy=Line'),
        ('nurbs_smooth',[boundary,nurbs],[pick(0,[-2.5,sqrt(18.75),0.]),pick(1,[.15,6.4725,0.])],'ExtendOtherCurvesBy=Smooth')]:
        for suffix,trim,join in [('parts',True,False),('joined',True,True),('bevel',False,False)]:
            rows['extension_'+label+'_'+suffix]=case(configured(distances=(.5,.2),trim=trim,join=join)+[option]+picks,curves=source)
    r=deepcopy(rows['extension_arc_parts']);r['actions']=r['actions'][:-1]+['Cancel'];r['pending_redo']=True;rows['extension_cancel_second']=r
    return rows

def holdouts(case,configured,pick):
    original=pilot(case,configured,pick);rows={}
    for label in ['arc','tangent_arc','nurbs_line','nurbs_smooth']:
        r=deepcopy(original['extension_'+label+'_joined']);r['actions'][-2:]=list(reversed(r['actions'][-2:]));rows['extension_'+label+'_reverse_picks']=r
    for label in ['tangent_arc','nurbs_line']:
        r=deepcopy(original['extension_'+label+'_joined']);r['actions'][2]=1.6;rows['extension_'+label+'_cross_source']=r
    r=deepcopy(original['extension_arc_joined']);r['curves'][1]['points'].reverse();rows['extension_arc_backward']=r
    r=deepcopy(original['extension_nurbs_smooth_joined']);r['curves'][1]['control_points'].reverse();rows['extension_nurbs_backward']=r
    def spatial(p):return [7.+.6*p[0],-2.+p[1],3.+.8*p[0]]
    r=deepcopy(original['extension_tangent_arc_joined']);r['curves'][0].update(center=spatial([0.,0.,0.]),x_axis=[.6,0.,.8],normal=[-.8,0.,.6]);r['curves'][1]['points']=[spatial(p) for p in r['curves'][1]['points']]
    for a in r['actions'][-2:]:a['pick']=spatial(a['pick'])
    rows['extension_spatial']=r
    r=deepcopy(original['extension_nurbs_line_parts']);r['curves'][1]['control_points']=[dict(point=p,weight=1.) for p in [[0.,6.,0.],[1.,6.,0.],[2.,7.,0.]]];r['actions'][-1]=pick(1,[.3,6.0225,0.]);r['pending_redo']=True;rows['extension_no_boundary_hit']=r
    return rows


def styling(case,configured,pick):
    base=pilot(case,configured,pick);space=holdouts(case,configured,pick)['extension_spatial'];rows={}
    r=deepcopy(space);r['actions'].insert(7,'ExtendOtherCurvesBy=Line');rows['extension_spatial_both_line']=r
    r=deepcopy(space);r['actions'][6]='ExtendArcsBy=Arc';r['actions'].insert(7,'ExtendOtherCurvesBy=Line');rows['extension_spatial_arc_other_line']=r
    r=deepcopy(base['extension_tangent_arc_joined']);r['actions'].insert(7,'ExtendOtherCurvesBy=Smooth');rows['extension_xy_line_other_smooth']=r
    r=deepcopy(space);r['construction_plane']=[[7.,-2.,3.],[.6,0.,.8],[0.,1.,0.]];rows['extension_aligned_plane_line']=r
    r=deepcopy(base['extension_tangent_arc_joined']);r['curves'][0]['center'][2]=3.
    r['curves'][1]['points']=[[p[0],p[1],p[2]+3.] for p in r['curves'][1]['points']]
    for a in r['actions'][-2:]:a['pick'][2]+=3.
    r['actions'].insert(7,'ExtendOtherCurvesBy=Smooth');rows['extension_translated_xy_line']=r
    def rotate(p):return [p[0],.6*p[1],.8*p[1]]
    r=deepcopy(base['extension_tangent_arc_joined']);r['curves'][0].update(normal=[0.,-.8,.6]);r['curves'][1]['points']=[rotate(p) for p in r['curves'][1]['points']]
    for a in r['actions'][-2:]:a['pick']=rotate(a['pick'])
    r['actions'].insert(7,'ExtendOtherCurvesBy=Smooth');rows['extension_tilted_x_line']=r
    return rows


def verification(case,configured,pick):
    """Explicit options isolate continuation geometry from remembered settings."""
    base=pilot(case,configured,pick);rows={}
    for label in ['arc','tangent_arc','nurbs_line']:
        r=deepcopy(base['extension_'+label+'_joined']);r['actions'][2]=1.6
        rows['extension_control_'+label+'_cross_source']=r
    for label in ['tangent_arc','nurbs_line']:
        r=deepcopy(base['extension_'+label+'_joined'])
        if label=='tangent_arc':r['curves'][1]['points'].reverse()
        else:r['curves'][1]['control_points'].reverse()
        rows['extension_control_'+label+'_backward']=r
    def spatial(p):return [7.+.6*p[0],-2.+p[1],3.+.8*p[0]]
    for label in ['arc','nurbs_line','nurbs_smooth']:
        r=deepcopy(base['extension_'+label+'_joined']);r['curves'][0].update(center=spatial([0.,0.,0.]),x_axis=[.6,0.,.8],normal=[-.8,0.,.6])
        c=r['curves'][1]
        if c['type']=='arc':c['points']=[spatial(p) for p in c['points']]
        else:
            for cp in c['control_points']:cp['point']=spatial(cp['point'])
        for a in r['actions'][-2:]:a['pick']=spatial(a['pick'])
        rows['extension_control_'+label+'_spatial']=r
    for r in rows.values():
        arc='Arc' if r['actions'][6]=='ExtendArcsBy=Arc' else 'Line'
        other='Smooth' if r['actions'][6]=='ExtendOtherCurvesBy=Smooth' else 'Line'
        r['actions'][6:7]=['ExtendArcsBy='+arc,'ExtendOtherCurvesBy='+other]
    return rows
