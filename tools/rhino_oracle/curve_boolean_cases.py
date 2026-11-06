"""Owned CurveBoolean choices, planar cells and source deletion contracts."""
from copy import deepcopy
if __package__:
    from .planar_regions_cases import rectangle,circle
else:
    from planar_regions_cases import rectangle,circle


def pilot():
    rows=[];curves=[rectangle(0.,0.,6.,6.),rectangle(4.,2.,10.,8.)]
    def add(name,options,points=None,all_regions=False):
        rows.append(dict(op='curve_boolean',id='curve_boolean_'+name,curves=deepcopy(curves),options=options,points=points or [],all_regions=all_regions))
    add('defaults',{},all_regions=True)
    for output in ['Curves','Surfaces']:
        for combine in [False,True]:
            add(output.lower()+'_'+str(combine),dict(delete_input='None',combine=combine,output=output,simplify=False),all_regions=True)
    for combine in [False,True]:
        add('picks_'+str(combine),dict(delete_input='None',combine=combine,output='Curves',simplify=False),[[1.,1.,0.],[5.,3.,0.]])
    add('toggle_empty',dict(delete_input='All',combine=False,output='Curves',simplify=False),[[1.,1.,0.],[1.,1.,0.]])
    for name,points,all_regions,combine in [('left',[[1.,1.,0.]],False,False),('overlap',[[5.,3.,0.]],False,False),('all',[],True,False),('union',[],True,True)]:
        add('used_'+name,dict(delete_input='Used',combine=combine,output='Curves',simplify=False),points,all_regions)
    add('all_delete',dict(delete_input='All',combine=False,output='Curves',simplify=True),all_regions=True)
    return rows


def holdouts():
    rows=[]
    def line(a,b):return dict(degree=1,knots=[0.,0.,1.,1.],control_points=[dict(point=p,weight=1.)for p in [a,b]])
    def add(name,curves,points=None,combine=False,delete='None',output='Curves'):
        rows.append(dict(op='curve_boolean',id='curve_boolean_holdout_'+name,curves=deepcopy(curves),options=dict(delete_input=delete,combine=combine,output=output,simplify=False),points=points or [],all_regions=points is None))
    outer=rectangle(0.,0.,10.,10.);inner=rectangle(2.,2.,8.,8.)
    for combine in [False,True]:
        add('nested_'+str(combine),[outer,inner],combine=combine,output='Surfaces')
        add('circles_'+str(combine),[circle(0.,0.,5.),circle(4.,0.,5.)],combine=combine)
    add('annulus',[outer,inner],[[1.,1.,0.]],output='Surfaces')
    add('bridge',[outer,inner,line([0.,5.,0.],[2.,5.,0.])],[[1.,1.,0.]],output='Surfaces')
    add('dangling',[outer,line([0.,5.,0.],[5.,5.,0.])],output='Surfaces')
    add('cross',[outer,line([-1.,5.,0.],[11.,5.,0.]),line([5.,-1.,0.],[5.,11.,0.])],output='Surfaces')
    add('open_triangle',[line([0.,0.,0.],[10.,0.,0.]),line([10.,0.,0.],[0.,10.,0.]),line([0.,10.,0.],[0.,0.,0.])])
    add('disjoint',[outer,rectangle(12.,0.,16.,4.)],combine=True)
    add('point_contact',[rectangle(0.,0.,4.,4.),rectangle(4.,4.,8.,8.)],combine=True)
    add('other_plane',[rectangle(0.,0.,6.,6.,3.),rectangle(4.,2.,10.,8.,3.)])
    add('different_planes',[rectangle(0.,0.,6.,6.),rectangle(4.,2.,10.,8.,3.)])
    tilted=[deepcopy(outer),deepcopy(inner)]
    for c in tilted:
        for p in c['control_points']:
            x,y,z=p['point'];p['point']=[.6*x,y,-.8*x+9.]
    add('tilted',tilted,output='Surfaces')
    add('no_region',[line([0.,0.,0.],[1.,0.,0.]),line([0.,0.,0.],[0.,1.,0.])])
    return rows


def interactions():
    rows=[]
    base=[rectangle(0.,0.,6.,6.),rectangle(4.,2.,10.,8.)]
    for preselect in [False,True]:
        for cancel in [False,True]:
            for delete in ['None','All','Used']:
                rows.append(dict(op='curve_boolean',id='curve_boolean_interaction_'+str(preselect)+'_'+str(cancel)+'_'+delete,
                    curves=deepcopy(base),options=dict(delete_input=delete,combine=True,output='Curves',simplify=True),points=[[1.,1.,0.]],all_regions=False,preselect=preselect,cancel=cancel))
    def add(name,curves,points=None,delete='None'):
        rows.append(dict(op='curve_boolean',id='curve_boolean_interaction_'+name,curves=deepcopy(curves),options=dict(delete_input=delete,combine=False,output='Curves',simplify=False),points=points or [],all_regions=points is None))
    add('reverse_planes',[rectangle(4.,2.,10.,8.,3.),rectangle(0.,0.,6.,6.)])
    add('used_projection',[rectangle(0.,0.,6.,6.),rectangle(4.,2.,10.,8.,3.)],[[1.,1.,0.]],'Used')
    add('single',[rectangle(0.,0.,6.,6.)])
    add('boundary',base,[[4.,3.,0.]])
    add('exterior',base,[[-1.,-1.,0.]])
    add('first_line',[dict(degree=1,knots=[0.,0.,1.,1.],control_points=[dict(point=p,weight=1.)for p in [[0.,0.,0.],[10.,0.,0.]]]),rectangle(2.,2.,8.,8.)])
    return rows


def confirmations():
    from math import sqrt
    rows=[]
    def add(name,curves,combine=False,points=None,delete='None',**extra):
        row=dict(op='curve_boolean',id='curve_boolean_confirmation_'+name,curves=deepcopy(curves),options=dict(delete_input=delete,combine=combine,output='Surfaces',simplify=False),points=points or [],all_regions=points is None)
        row.update(extra);rows.append(row)
    circles=[circle(0.,0.,5.),circle(4.,0.,5.)]
    for combine in [False,True]:add('precise_circles_'+str(combine),circles,combine)
    add('three_circles',[circle(0.,0.,5.),circle(4.,0.,5.),circle(2.,3.,5.)])
    outer=rectangle(0.,0.,10.,10.);inner=rectangle(2.,2.,8.,8.)
    add('nested_three',[outer,inner,rectangle(3.,3.,7.,7.)])
    add('duplicate',[outer,outer])
    add('overlap_edge',[outer,rectangle(10.,2.,14.,6.)],True)
    add('self_cross',[dict(degree=1,knots=[0.,0.,1.,2.,3.,4.,4.],control_points=[dict(point=p,weight=1.)for p in [[0.,0.,0.],[10.,10.,0.],[0.,10.,0.],[10.,0.,0.],[0.,0.,0.]]])])
    weighted=dict(degree=1,knots=[12.,12.,20.,20.],control_points=[dict(point=[0.,0.,0.],weight=1.),dict(point=[4.,0.,0.],weight=4.)])
    side=dict(degree=1,knots=[0.,0.,1.,2.,3.,3.],control_points=[dict(point=p,weight=1.)for p in [[4.,0.,0.],[4.,4.,0.],[0.,4.,0.],[0.,0.,0.]]])
    add('rational_line',[weighted,side,rectangle(1.,-2.,3.,1.)],True,delete='Used')
    for scale,label in [(1e-3,'small'),(1e3,'large')]:
        shapes=deepcopy(circles)
        for c in shapes:
            for p in c['control_points']:p['point']=[scale*p['point'][0]+31.,scale*p['point'][1]-17.,5.]
        add('scaled_'+label,shapes)
    add('cplane_projection',[rectangle(0.,0.,6.,6.),rectangle(4.,2.,10.,8.,3.)],cplane=dict(origin=[0.,0.,7.],x_axis=[1.,0.,0.],y_axis=[0.,1.,0.]))
    add('coplanar_ignores_cplane',[outer,inner],cplane=dict(origin=[0.,0.,7.],x_axis=[1.,0.,0.],y_axis=[0.,1.,0.]))
    add('cancel_remembers',[outer,inner],cancel=True,check_preferences=True)
    return rows


def weighted_deletion():
    base=next(r for r in confirmations()if r['id']=='curve_boolean_confirmation_rational_line')
    rows=[]
    for name,weights,knots in [('reversed_weights',[4.,1.],[12.,12.,20.,20.]),('other_domain',[1.,4.],[-7.,-7.,3.,3.])]:
        r=deepcopy(base);r['id']='curve_boolean_weighted_'+name
        for p,w in zip(r['curves'][0]['control_points'],weights):p['weight']=w
        r['curves'][0]['knots']=knots;rows.append(r)
    r=deepcopy(pilot()[8]);r['id']='curve_boolean_weighted_multiple_spans'
    for p,w in zip(r['curves'][0]['control_points'],[1.,2.,3.,4.,1.]):p['weight']=w
    rows.append(r)
    return rows


def validate(op):
    def same(a,b):
        if isinstance(b,dict):return isinstance(a,dict)and set(a)==set(b)and all(same(a[k],v)for k,v in b.items())
        if isinstance(b,list):return isinstance(a,list)and len(a)==len(b)and all(same(x,y)for x,y in zip(a,b))
        if type(b)in (float,int):return type(a)in (float,int)and a==b
        return type(a)==type(b)and a==b
    own=next((r for r in pilot()+holdouts()+interactions()+confirmations()+weighted_deletion()if isinstance(op,dict)and r['id']==op.get('id')),None)
    if own is None or not same(op,own):raise ValueError('fixed owned CurveBoolean recipe required')
