"""Zero-radius closed-source Fillet recipes, authored before native capture."""
from copy import deepcopy
from math import cos, sin, sqrt, pi


def pilot(case, explicit, pick, closed_pilot):
    rows = {}
    for name, recipe in closed_pilot.items():
        r = deepcopy(recipe)
        r['actions'][1] = 0.
        rows['zero_' + name] = r
    r = deepcopy(rows['zero_circle_line_joined'])
    r['curves'][1].update(start=[0., 6., 0.], end=[0., 8., 0.])
    rows['zero_line_extension'] = r
    r = deepcopy(rows['zero_circle_line_parts'])
    r['curves'][1].update(start=[6., -8., 0.], end=[6., 8., 0.])
    r['actions'][-1] = pick(1, [6., 6.8, 0.])
    r['pending_redo'] = True
    rows['zero_disjoint_line'] = r
    for label, tail in [('accept', ['Enter']), ('cancel', ['Cancel'])]:
        r = deepcopy(rows['zero_circle_line_joined'])
        r['actions'][5] = 'Dynamic=Yes'
        r['actions'] += tail
        if label == 'cancel': r['pending_redo'] = True
        rows['zero_dynamic_' + label] = r
    return rows


def verification(case, explicit, pick, closed_pilot, closed_final, continuations):
    base = holdouts(case, explicit, pick, closed_pilot, closed_final, continuations)
    rows = {}
    for label, trim in [('parts',True),('untrimmed',False)]:
        r = deepcopy(base['zero_two_circles_disjoint'])
        r['actions'][3] = 'Join=No'; r['actions'][4] = 'Trim='+('Yes'if trim else'No')
        rows['zero_disjoint_circles_' + label] = r
    for label, angle, y in [('metric_upper',pi-.2,-6.8),('metric_lower',pi+.2,6.8)]:
        r = deepcopy(base['zero_opposed'])
        r['actions'][-2:] = [pick(0,[5.*cos(angle),5.*sin(angle),0.]),pick(1,[0.,y,0.])]
        rows['zero_' + label] = r
    r = deepcopy(rows['zero_metric_upper'])
    r['actions'][-2:] = list(reversed(r['actions'][-2:])); rows['zero_metric_reverse'] = r
    r = deepcopy(base['zero_nurbs_circle_line'])
    r['actions'][-2:] = [pick(0,[5.*cos(pi-.2),5.*sin(pi-.2),0.]),pick(1,[0.,-6.8,0.])]
    rows['zero_metric_nurbs'] = r
    return rows


def holdouts(case, explicit, pick, closed_pilot, closed_final, continuations):
    base = pilot(case, explicit, pick, closed_pilot)
    rows = {}
    for name in ['circle_line_joined_reverse', 'circle_line_lower', 'circle_line_inner',
                 'nurbs_circle_line', 'ellipse_line', 'circle_line_spatial_top']:
        r = deepcopy(closed_final[name]); r['actions'][1] = 0.
        rows['zero_' + name] = r
    for label, angle, y in [('opposed', .35, -6.8), ('seam_opposed', -.35, 6.8)]:
        r = deepcopy(base['zero_circle_line_parts'])
        r['actions'][-2:] = [pick(0, [5.*cos(angle),5.*sin(angle),0.]),pick(1,[0.,y,0.])]
        rows['zero_' + label] = r
    r = deepcopy(base['zero_circle_line_parts'])
    r['curves'][1].update(start=[5.,-8.,0.],end=[5.,8.,0.])
    r['actions'][-2:] = [pick(0,[4.,3.,0.]),pick(1,[5.,3.,0.])]
    rows['zero_tangent_line'] = r
    for suffix in ['parts','joined']:
        r = deepcopy(base['zero_circle_line_' + suffix])
        r['curves'][1].update(start=[0.,5.,0.],end=[0.,8.,0.])
        r['pending_redo'] = True
        rows['zero_meeting_' + suffix] = r
    r = deepcopy(base['zero_disjoint_line']); r['actions'][4] = 'Trim=No'
    rows['zero_disjoint_untrimmed'] = r
    r = deepcopy(base['zero_two_circles_joined'])
    r['actions'][-2:] = list(reversed(r['actions'][-2:])); r['pending_redo'] = True
    rows['zero_two_circles_joined_reverse'] = r
    r = deepcopy(base['zero_two_circles_joined']); r['curves'][1]['center'] = [12.,0.,0.]
    r['actions'][-1] = pick(1,[7.5,sqrt(4.75),0.]); r['pending_redo'] = True
    rows['zero_two_circles_disjoint'] = r
    r = deepcopy(base['zero_circle_line_parts'])
    r['curves'][1].update(start=[0.,2.,-3.],end=[0.,8.,3.])
    r['actions'][-1] = pick(1,[0.,6.8,1.8]); rows['zero_noncoplanar_intersection'] = r
    r = deepcopy(base['zero_circle_line_parts'])
    r['curves'][1].update(start=[0.,-8.,.5],end=[0.,8.,.5])
    r['actions'][-1] = pick(1,[0.,6.8,.5]); r['pending_redo'] = True
    rows['zero_skew_line'] = r
    for label in ['arc','tangent_arc','nurbs_line','nurbs_smooth']:
        r = deepcopy(continuations['continuation_' + label + '_joined']); r['actions'][1] = 0.
        rows['zero_continuation_' + label] = r
    r = deepcopy(base['zero_circle_line_parts'])
    r['curves'][0] = dict(type='polyline',vertices=[[-5.,-5.,0.],[5.,-5.,0.],[5.,5.,0.],[-5.,5.,0.],[-5.,-5.,0.]])
    r['actions'][-2] = pick(0,[2.5,5.,0.]); rows['zero_rectangle_line'] = r
    r = deepcopy(base['zero_circle_line_parts'])
    r['curves'][1] = dict(type='nurbs',degree=2,control_points=[dict(point=p,weight=1.)for p in [[-1.5,-8.,0.],[0.,-8.,0.],[1.5,8.,0.]]],knots=[0.,0.,0.,1.,1.,1.])
    r['actions'][-1] = pick(1,[1.35,6.44,0.]); rows['zero_nonlinear_nurbs'] = r
    for label, start, end in [('positive_to_zero',.5,0.),('zero_to_positive',0.,.5)]:
        r = deepcopy(base['zero_dynamic_accept']); r['actions'][1] = start
        r['actions'][-1:] = [end,'Enter']; rows['zero_' + label] = r
    return rows
