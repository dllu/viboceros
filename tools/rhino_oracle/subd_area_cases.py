"""Owned SubD area sources, independent of measured native outputs."""
import math
if __package__:
    from .component_length_cases import source as edge_source
else:
    from component_length_cases import source as edge_source


def source(scene):
    if scene=='pentagon':
        return dict(points=[[3*math.cos(2*math.pi*i/5),3*math.sin(2*math.pi*i/5),.4*math.sin(4*math.pi*i/5)]for i in range(5)],faces=[list(range(5))],corners=list(range(5)))
    if scene in ('fan7','asymmetric8'):
        n=7 if scene=='fan7' else 8
        points=[[0.,0.,.6]]
        for i in range(n):
            a=2*math.pi*i/n;points.append([1.5*math.cos(a),1.5*math.sin(a),.15*math.sin(2*a)])
        for i in range(n):
            a=2*math.pi*(i+.5)/n;points.append([3*math.cos(a),3*math.sin(a),.45*math.cos(3*a)])
        if scene=='asymmetric8':points[1]=[1.2,.2,.7];points[n+2][2]+=.35
        return dict(points=points,faces=[[0,1+i,1+n+i,1+(i+1)%n]for i in range(n)],corners=list(range(n+1,2*n+1)))
    return edge_source(scene)


def cases():
    return [dict(op='subd_area',id=scene+'_'+str(int(flat)),scene=scene,flat=flat)
            for scene in ('grid','bent_grid','creased_grid','fan5','raised_fan','cube')
            for flat in (False,True)]


def validate(op):
    if not isinstance(op,dict) or op not in cases()+followups() or type(op.get('flat')) is not bool or ('curve'in op and type(op['curve'])is not bool) or ('preselect'in op and type(op['preselect'])is not bool):
        raise ValueError('fixed owned SubD area case required')


def followups():
    rows=[dict(op='subd_area',id=scene,scene=scene,flat=False)for scene in ('fan7','asymmetric8','pentagon')]
    for scene in ('bent_grid','cube'):
        rows.append(dict(op='subd_area',id=scene+'_mixed',scene=scene,flat=False,curve=True))
        rows.append(dict(op='subd_area',id=scene+'_postselected',scene=scene,flat=False,preselect=False))
    return rows
