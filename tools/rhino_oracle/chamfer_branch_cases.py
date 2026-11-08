"""Nonmeeting end-selection controls, authored independently of native output."""
from copy import deepcopy
from math import cos,sin


def cases(base):
    rows={}
    for label in ['arc','tangent_arc','nurbs_line','nurbs_smooth']:
        r=deepcopy(base['extension_'+label+'_joined'])
        arc='Arc' if label=='arc' else 'Line';other='Smooth' if label=='nurbs_smooth' else 'Line'
        r['actions'][6:7]=['ExtendArcsBy='+arc,'ExtendOtherCurvesBy='+other]
        r['actions'][-1]['pick']=[2.*cos(1.35),5.+2.*sin(1.35),0.] if 'arc' in label else [.8,9.04,0.]
        rows['branch_'+label+'_far_pick']=r
        r=deepcopy(r);r['actions'][-2:]=list(reversed(r['actions'][-2:]));rows['branch_'+label+'_far_reverse']=r
    for label in ['arc','tangent_arc']:
        r=deepcopy(base['extension_'+label+'_joined']);r['actions'][6:7]=['ExtendArcsBy='+('Arc'if label=='arc'else'Line'),'ExtendOtherCurvesBy=Line']
        r['actions'][-2]['pick']=[-3.,4.,0.]
        rows['branch_'+label+'_left_boundary_pick']=r
    for label in ['tangent_arc','nurbs_line']:
        r=deepcopy(rows['branch_'+label+'_far_pick']);r['actions'][4]='Join=No';rows['branch_'+label+'_far_parts']=r
    return rows


def holdouts(base):
    pilot=cases(base);rows={}
    for label in ['arc','tangent_arc','nurbs_line','nurbs_smooth']:
        r=deepcopy(pilot['branch_'+label+'_far_pick']);c=r['curves'][1]
        if c['type']=='arc':c['points'].reverse()
        else:c['control_points'].reverse()
        rows['branch_'+label+'_backward_far']=r
        r=deepcopy(pilot['branch_'+label+'_far_pick']);r['curves'][0]=dict(type='polyline',vertices=[[-4.,-1.,0.],[4.,-1.,0.],[4.,4.,0.],[-4.,4.,0.],[-4.,-1.,0.]])
        r['actions'][-2]['pick']=[2.5,4.,0.]
        rows['branch_'+label+'_rectangle']=r
    for label in ['nurbs_line','nurbs_smooth']:
        r=deepcopy(pilot['branch_'+label+'_far_pick']);r['curves'][0]=dict(type='ellipse',center=[0.,0.,0.],radius_x=5.,radius_y=3.,x_axis=[1.,0.,0.],y_axis=[0.,1.,0.])
        r['actions'][-2]['pick']=[-2.5,3.**.5*1.5,0.]
        rows['branch_'+label+'_ellipse']=r
    for label in ['arc','tangent_arc']:
        r=deepcopy(base['extension_'+label+'_joined']);r['actions'][6:7]=['ExtendArcsBy='+('Arc'if label=='arc'else'Line'),'ExtendOtherCurvesBy=Line']
        r['curves'][0].update(center=[-4.,7.,0.],radius=2.5);r['actions'][-2]['pick']=[-2.,8.5,0.]
        rows['branch_'+label+'_end_only']=r
    r=deepcopy(pilot['branch_nurbs_line_far_pick']);r['curves'][1]['control_points']=[dict(point=p,weight=1.)for p in [[0.,6.,0.],[1.,6.,0.],[2.,7.,0.]]];r['actions'][-1]['pick']=[1.6,6.64,0.];r['pending_redo']=True
    rows['branch_both_ends_miss']=r
    r=deepcopy(pilot['branch_tangent_arc_far_pick']);r['actions'][-1]='Cancel';r['pending_redo']=True;rows['branch_cancel_far_pick']=r
    return rows


def seams(base):
    from math import sqrt
    rows={}
    for kind in ['circle','backward','nurbs','ellipse']:
        for sign in [-1.,1.]:
            r=deepcopy(base['extension_tangent_arc_joined']);r['actions'][6:7]=['ExtendArcsBy=Line','ExtendOtherCurvesBy=Line']
            y=sign*(.6 if kind=='ellipse'else 1.)
            r['curves'][1]=dict(type='line',start=[6.,y,0.],end=[10.,y,0.])
            r['actions'][-2]['pick']=[4.,-sign*(1.8 if kind=='ellipse'else 3.),0.]
            r['actions'][-1]['pick']=[7.,y,0.]
            if kind=='backward':r['curves'][0]['normal']=[0.,0.,-1.]
            elif kind=='nurbs':
                points=[[5.,0.,0.],[5.,5.,0.],[0.,5.,0.],[-5.,5.,0.],[-5.,0.,0.],[-5.,-5.,0.],[0.,-5.,0.],[5.,-5.,0.],[5.,0.,0.]]
                r['curves'][0]=dict(type='nurbs',degree=2,control_points=[dict(point=p,weight=1. if i%2==0 else sqrt(.5))for i,p in enumerate(points)],knots=[0.,0.,0.,1.,1.,2.,2.,3.,3.,4.,4.,4.])
            elif kind=='ellipse':r['curves'][0]=dict(type='ellipse',center=[0.,0.,0.],radius_x=5.,radius_y=3.,x_axis=[1.,0.,0.],y_axis=[0.,1.,0.])
            rows['branch_seam_'+kind+('_below'if sign<0 else'_above')]=r
    return rows


def verification(base):
    original=seams(base);rows={}
    for kind in ['circle','nurbs','ellipse']:
        for suffix in ['below','above']:
            r=deepcopy(original['branch_seam_'+kind+'_'+suffix]);r['actions'][-2:]=list(reversed(r['actions'][-2:]));rows['branch_seam_'+kind+'_'+suffix+'_reverse_picks']=r
    for suffix in ['below','above']:
        r=deepcopy(original['branch_seam_circle_'+suffix]);r['curves'][1]['start'][0]=-10.;rows['branch_seam_crossing_'+suffix]=r
        r=deepcopy(original['branch_seam_circle_'+suffix]);r['curves'][0]['x_axis']=[-1.,0.,0.];rows['branch_seam_shifted_'+suffix]=r
        r=deepcopy(original['branch_seam_circle_'+suffix]);c=r['curves'][1];c['start'],c['end']=c['end'],c['start'];rows['branch_seam_line_backward_'+suffix]=r
    return rows
