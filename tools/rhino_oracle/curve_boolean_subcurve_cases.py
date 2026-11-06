"""Owned hidden CurveBoolean SubCrv selections and parent-deletion controls."""
from copy import deepcopy
if __package__:
    from .planar_regions_cases import rectangle
else:
    from planar_regions_cases import rectangle


def path(points):
    n=len(points)
    return dict(degree=1,knots=[0.]+[float(i)for i in range(n)]+[float(n-1)],control_points=[dict(point=p,weight=1.)for p in points])


def pilot():
    rows=[]
    def add(name,curves,selections,delete='None'):
        rows.append(dict(op='curve_boolean',id='curve_boolean_subcurve_'+name,curves=deepcopy(curves),
            options=dict(delete_input=delete,combine=False,output='Surfaces',simplify=False),points=[],all_regions=True,preselect=False,selections=deepcopy(selections)))
    def partial(source,a,b):return dict(source=source,range=[a,b])
    def whole(source):return dict(source=source,range=None)
    open_curves=[path([[-5.,0.,0.],[10.,0.,0.],[10.,10.,0.],[0.,10.,0.],[0.,-5.,0.]]),path([[0.,10.,0.],[10.,0.,0.]])]
    for delete in ['None','All','Used']:
        add('open_'+delete,open_curves,[partial(0,[10.,0.,0.],[0.,10.,0.]),whole(1)],delete)
    add('open_reverse',open_curves,[partial(0,[0.,10.,0.],[10.,0.,0.]),whole(1)])
    closed=[rectangle(0.,0.,10.,10.),path([[5.,0.,0.],[0.,5.,0.]])]
    for delete in ['None','All','Used']:
        add('wrap_'+delete,closed,[partial(0,[0.,5.,0.],[5.,0.,0.]),whole(1)],delete)
    add('long_closed',closed,[partial(0,[5.,0.,0.],[0.,5.,0.]),whole(1)])
    add('repeated_parent',open_curves,[partial(0,[0.,0.,0.],[0.,10.,0.]),partial(0,[10.,0.,0.],[0.,0.,0.]),whole(1)],'Used')
    add('whole_and_partial',open_curves,[whole(0),partial(0,[10.,0.,0.],[0.,10.,0.]),whole(1)],'All')
    return rows


def holdouts():
    rows=[]
    base=pilot()[0]
    for label,length,confirm in [('numeric',20.,[0.,10.,0.]),('negative_numeric',-20.,[0.,10.,0.])]:
        r=deepcopy(base);r['id']='curve_boolean_subcurve_'+label
        r['selections'][0]['length']=length;r['selections'][0]['range'][1]=confirm;rows.append(r)
    for length,label in [(0.,'zero'),(40.,'full_closed')]:
        r=deepcopy(pilot()[4]);r['id']='curve_boolean_subcurve_'+label
        r['curves'][1]=rectangle(20.,0.,22.,2.)
        r['selections'][0]['range']=[[5.,0.,0.],[10.,0.,0.]]
        r['selections'][0]['length']=length
        r['selections'][0]['confirmation']=length!=0.
        rows.append(r)
    for phase in ['source','start','end']:
        r=deepcopy(base);r['id']='curve_boolean_subcurve_cancel_'+phase;r['cancel_subcurve']=phase;r['cancel']=True;rows.append(r)
    return rows


def edges():
    surface=dict(degree_u=1,degree_v=1,control_point_count_u=2,control_point_count_v=2,
        knots_u=[0.,0.,1.,1.],knots_v=[0.,0.,1.,1.],
        control_points=[dict(point=p,weight=1.)for p in [[0.,0.,0.],[10.,0.,0.],[0.,10.,0.],[10.,10.,0.]]])
    rows=[]
    for delete in ['None','All','Used']:
        rows.append(dict(op='curve_boolean',id='curve_boolean_subcurve_edge_'+delete,
            curves=[path([[8.,0.,0.],[8.,4.,0.],[2.,4.,0.],[2.,0.,0.]])],surfaces=[deepcopy(surface)],
            options=dict(delete_input=delete,combine=False,output='Surfaces',simplify=False),points=[],all_regions=True,preselect=False,
            selections=[dict(source=1,edge=0,range=[[2.,0.,0.],[8.,0.,0.]]),dict(source=0,range=None)]))
    reverse=deepcopy(rows[0]);reverse['id']='curve_boolean_subcurve_edge_reverse';reverse['selections'][0]['range'].reverse();rows.append(reverse)
    return rows
