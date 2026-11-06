"""Crossing inner boundaries with independently moved stored seams."""
from copy import deepcopy
if __package__:
    from .planar_regions_cases import rectangle,circle
else:
    from planar_regions_cases import rectangle,circle


def seam(curve,index):
    result=deepcopy(curve);points=result['control_points'][:-1]
    shift=index*result['degree'];points=points[shift:]+points[:shift]
    result['control_points']=points+[deepcopy(points[0])]
    return result


def pilot():
    rows=[]
    for label,curves in [
        ('rect',[rectangle(0.,0.,10.,10.),rectangle(2.,2.,6.,6.),rectangle(4.,4.,8.,8.)]),
        ('bars',[rectangle(0.,0.,10.,10.),rectangle(1.,4.,9.,6.),rectangle(4.,1.,6.,9.)]),
        ('circle',[circle(0.,0.,10.),circle(-2.,0.,4.),circle(2.,0.,4.)])]:
        for a in range(4):
            for b in range(4):
                rows.append(dict(op='planar_regions',id='crossing_'+label+str(a)+str(b),
                                 curves=[deepcopy(curves[0]),seam(curves[1],a),seam(curves[2],b)]))
    return rows


def details():
    rows=[deepcopy(r)for r in pilot()if r['id']in ['crossing_rect01','crossing_bars00','crossing_circle10']]
    outer=rectangle(0.,0.,10.,10.);hole=rectangle(2.,2.,6.,6.);reverse=deepcopy(hole);reverse['control_points'].reverse()
    for label,curves in [('duplicate',[outer,hole,reverse]),('hole',[outer,hole]),('outer',[outer]),
                         ('touch',[outer,rectangle(1.,1.,4.,4.),rectangle(4.,1.,7.,4.)]),
                         ('separate',[outer,rectangle(1.,1.,4.,4.),rectangle(6.,6.,9.,9.)])]:
        rows.append(dict(op='planar_regions',id=label,curves=deepcopy(curves)))
    for row in rows:
        row['id']='crossing_details_'+row['id']
        row['samples']=[[x,y,0.]for y in [.5,2.5,3.5,4.5,5.,5.5,6.5,8.5]for x in [.5,2.5,3.5,4.5,5.,5.5,6.5,8.5]]
    return rows


def holdouts():
    rows=[]
    chosen=[r for r in pilot()if r['id']in ['crossing_rect01','crossing_bars00','crossing_circle10']]
    for source in chosen:
        for label in ['reverse_a','reverse_b','reverse_both','order','domain','gauge','small','large','tilted','shear','refined']:
            row=deepcopy(source);row['id']=source['id']+'_'+label
            if label.startswith('reverse'):
                for i in ([1,2]if label=='reverse_both'else[1 if label=='reverse_a'else 2]):row['curves'][i]['control_points'].reverse()
            if label=='order':row['curves']=[row['curves'][2],row['curves'][0],row['curves'][1]]
            for i,curve in enumerate(row['curves']):
                if label=='domain':curve['knots']=[123.+i+(i+.25)*k for k in curve['knots']]
                if label=='gauge':
                    for p in curve['control_points']:p['weight']*=7.
                for p in curve['control_points']:
                    x,y,z=p['point']
                    if label=='small':p['point']=[.001*x+2.,.001*y-3.,.001*z+4.]
                    if label=='large':p['point']=[1000.*x+1e6,1000.*y-2e6,1000.*z+3e6]
                    if label=='tilted':p['point']=[.6*x+.8*z+31.,y-17.,-.8*x+.6*z+9.]
                    if label=='shear':p['point']=[1.2*x+.3*y+17.,.7*y-3.,z]
                if label=='refined'and curve['degree']==1:
                    points=curve['control_points'];curve['control_points']=[]
                    for a,b in zip(points,points[1:]):
                        curve['control_points'].extend([a,dict(point=[(x+y)/2. for x,y in zip(a['point'],b['point'])],weight=1.)])
                    curve['control_points'].append(deepcopy(points[-1]));n=len(curve['control_points']);curve['knots']=[0.]+[float(k)for k in range(n)]+[float(n-1)]
            rows.append(row)
    return rows
