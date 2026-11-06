"""Owned closed-loop nesting, overlap, contact and input-order recipes."""
from copy import deepcopy
from itertools import permutations


def polygon(points):
    points=deepcopy(points+[points[0]])
    n=len(points)
    return dict(degree=1,control_points=[dict(point=p,weight=1.)for p in points],knots=[0.]+[float(i)for i in range(n)]+[float(n-1)])


def rectangle(a,b,c,d,z=0.):
    return polygon([[a,b,z],[c,b,z],[c,d,z],[a,d,z]])


def circle(x,y,r):
    from math import sqrt
    points=[[1.,0.],[1.,1.],[0.,1.],[-1.,1.],[-1.,0.],[-1.,-1.],[0.,-1.],[1.,-1.],[1.,0.]]
    return dict(degree=2,knots=[0.]*3+[1.,1.,2.,2.,3.,3.]+[4.]*3,
                control_points=[dict(point=[x+r*a,y+r*b,0.],weight=1. if i%2==0 else sqrt(.5))for i,(a,b)in enumerate(points)])


def pilot():
    outer=rectangle(0.,0.,10.,10.);a=rectangle(2.,2.,6.,6.);b=rectangle(4.,4.,8.,8.)
    rows=[]
    def add(name,curves):rows.append(dict(op='planar_regions',id=name,curves=deepcopy(curves)))
    for label,profiles in [('equal',[outer,a,b]),('unequal',[outer,rectangle(1.,1.,6.,6.),b])]:
        for order in permutations(range(3)):add(label+'_order_'+''.join(map(str,order)),[profiles[i]for i in order])
    chain=[rectangle(1.,1.,5.,5.),rectangle(3.,3.,7.,7.),rectangle(5.5,5.5,9.5,9.5)]
    for order in permutations(range(3)):add('chain_'+''.join(map(str,order)),[outer]+[chain[i]for i in order])
    add('old_overlap',[outer,a,b,rectangle(.5,.5,1.5,1.5)])
    add('nested',[rectangle(i,i,10.-i,10.-i)for i in range(5)])
    add('nested_scrambled',[rectangle(i,i,10.-i,10.-i)for i in [3,0,4,1,2]])
    add('disconnected',[outer,rectangle(12.,0.,16.,4.),a])
    add('touching_siblings_edge',[outer,rectangle(1.,1.,4.,4.),rectangle(4.,1.,7.,4.)])
    add('touching_siblings_point',[outer,rectangle(1.,1.,4.,4.),rectangle(4.,4.,7.,7.)])
    add('touching_outer_edge',[outer,rectangle(0.,2.,3.,5.)])
    add('touching_outer_point',[outer,polygon([[0.,0.,0.],[3.,1.,0.],[1.,3.,0.]])])
    add('crossing_outer',[outer,rectangle(8.,4.,12.,8.)])
    add('two_outers_common_hole',[outer,rectangle(5.,0.,15.,10.),rectangle(6.,2.,8.,4.)])
    add('two_outers_left_hole',[outer,rectangle(5.,0.,15.,10.),rectangle(1.,2.,3.,4.)])
    add('two_outers_two_holes',[outer,rectangle(5.,0.,15.,10.),rectangle(6.,2.,8.,4.),rectangle(1.,6.,3.,8.)])
    add('duplicates',[outer,deepcopy(outer)])
    add('duplicate_holes',[outer,a,deepcopy(a)])
    add('duplicate_with_overlap',[outer,a,deepcopy(a),b])
    add('two_planes',[outer,a,rectangle(0.,0.,10.,10.,2.),rectangle(2.,2.,6.,6.,2.)])
    for reverse in [False,True]:
        for seam in range(4):
            points=[[2.,2.,0.],[6.,2.,0.],[6.,6.,0.],[2.,6.,0.]]
            if reverse:points.reverse()
            points=points[seam:]+points[:seam]
            add('seam_'+str(int(reverse))+str(seam),[outer,polygon(points),b])
    add('circles',[circle(0.,0.,10.),circle(-2.,0.,4.),circle(2.,0.,4.)])
    add('circles_reverse_order',[circle(0.,0.,10.),circle(2.,0.,4.),circle(-2.,0.,4.)])
    add('circle_nested',[circle(0.,0.,10.),circle(0.,0.,7.),circle(0.,0.,3.)])
    add('circle_external_tangent',[circle(0.,0.,10.),circle(-3.,0.,3.),circle(3.,0.,3.)])
    add('circle_internal_tangent',[circle(0.,0.,10.),circle(6.,0.,4.)])
    add('concave',[polygon([[0.,0.,0.],[10.,0.,0.],[10.,10.,0.],[5.,5.,0.],[0.,10.,0.]]),a,b])
    add('island_in_overlap',[outer,a,b,rectangle(4.5,4.5,5.5,5.5)])
    return rows


def holdouts():
    rows=[];outer=rectangle(0.,0.,10.,10.)
    def add(name,curves):rows.append(dict(op='planar_regions',id=name,curves=deepcopy(curves)))
    for label,points in [
        ('touch',[[0.,2.,0.],[3.,2.,0.],[3.,5.,0.],[0.,5.,0.]]),
        ('cross',[[8.,4.,0.],[12.,4.,0.],[12.,8.,0.],[8.,8.,0.]])]:
        for reverse in [False,True]:
            p=list(reversed(points))if reverse else points
            for seam in range(4):add(label+str(int(reverse))+str(seam),[outer,polygon(p[seam:]+p[:seam])])
    for seam in range(4):
        c=circle(6.,0.,4.);controls=c['control_points'][:-1];controls=controls[2*seam:]+controls[:2*seam];c['control_points']=controls+[deepcopy(controls[0])]
        add('internal_circle_seam'+str(seam),[circle(0.,0.,10.),c])
    for original in pilot():
        op=deepcopy(original);op['id']='precise_'+op['id'];rows.append(op)
    return rows


def transforms():
    rows=[]
    chosen=[r for r in pilot()if r['id']in ['old_overlap','nested_scrambled','circles','concave','island_in_overlap']]
    for original in chosen:
        for label,point in [('tilted',lambda x,y,z:[.6*x+.8*z+31.,y-17.,-.8*x+.6*z+9.]),
                            ('small',lambda x,y,z:[.001*x+2.,.001*y-3.,.001*z+4.]),
                            ('large',lambda x,y,z:[1000.*x+1e6,1000.*y-2e6,1000.*z+3e6])]:
            op=deepcopy(original);op['id']=original['id']+'_'+label
            for c in op['curves']:
                for p in c['control_points']:p['point']=point(*p['point'])
            rows.append(op)
        op=deepcopy(original);op['id']=original['id']+'_domains'
        for i,c in enumerate(op['curves']):c['knots']=[100.+i+(.25+i)*v for v in c['knots']]
        rows.append(op)
    outer=rectangle(0.,0.,10.,10.);hole=rectangle(2.,2.,6.,6.)
    for name,source in [('outer',outer),('hole',hole),('circle',circle(0.,0.,10.))]:
        for label in ['reverse','domain','seam','gauge']:
            duplicate=deepcopy(source)
            if label=='reverse':duplicate['control_points'].reverse()
            if label=='domain':duplicate['knots']=[123.+7.*v for v in duplicate['knots']]
            if label=='seam':
                controls=duplicate['control_points'][:-1];shift=duplicate['degree'];controls=controls[shift:]+controls[:shift];duplicate['control_points']=controls+[deepcopy(controls[0])]
            if label=='gauge':
                for p in duplicate['control_points']:p['weight']*=7.
            curves=[source,duplicate]if name!='hole'else[outer,source,duplicate]
            rows.append(dict(op='planar_regions',id='duplicate_'+name+'_'+label,curves=deepcopy(curves)))
    return rows


def planes():
    profiles=[rectangle(0.,0.,10.,10.),rectangle(2.,2.,6.,6.),polygon([[2.,0.,-5.],[2.,10.,-5.],[2.,10.,5.],[2.,0.,5.]])]
    rows=[]
    for order in permutations(range(3)):
        rows.append(dict(op='planar_regions',id='orthogonal_'+''.join(map(str,order)),curves=[deepcopy(profiles[i])for i in order]))
    rows.append(dict(op='planar_regions',id='orthogonal_pair',curves=deepcopy([profiles[0],profiles[2]])))
    rows.append(dict(op='planar_regions',id='orthogonal_both_holes',curves=deepcopy(profiles+[polygon([[2.,3.,-2.],[2.,5.,-2.],[2.,5.,2.],[2.,3.,2.]])])))
    return rows


def validate(op):
    def same(a,b):
        if isinstance(b,dict):return isinstance(a,dict)and set(a)==set(b)and all(same(a[k],v)for k,v in b.items())
        if isinstance(b,list):return isinstance(a,list)and len(a)==len(b)and all(same(x,y)for x,y in zip(a,b))
        if isinstance(b,(float,int)):return type(a)in (float,int)and a==b
        return type(a)==type(b)and a==b
    owned=next((r for r in pilot()+holdouts()+transforms()+planes()if isinstance(op,dict)and r['id']==op.get('id')),None)
    if owned is None or not same(op,owned):raise ValueError('fixed owned planar-region recipe required')
