"""Owned independent continuity profiles, cap modes and split policies."""
from copy import deepcopy
if __package__:
    from .revolve_subd_cases import profiles
    from .extrude_apex_subd_cases import holdouts as apex_profiles
else:
    from revolve_subd_cases import profiles
    from extrude_apex_subd_cases import holdouts as apex_profiles


def pilot():
    source=profiles();rows=[]
    for name in ['smooth','tangent','corner','polyline','rational','closed']:
        for crease in [False,True]:
            for split in [False,True]:
                rows.append(dict(op='apex_splitting',id=name+'_'+str(int(crease))+str(int(split)),curve=deepcopy(source[name]),apex=[0.,3.,2.],solid=False,delete_input=split,crease_splitting=crease,split_at_tangents=split))
    for crease in [False,True]:
        for split in [False,True]:
            rows.append(dict(op='apex_splitting',id='closed_solid_'+str(int(crease))+str(int(split)),curve=deepcopy(source['closed']),apex=[0.,3.,2.],solid=True,delete_input=split,crease_splitting=crease,split_at_tangents=split))
    return rows


def holdouts():
    source=profiles();source.update({r['id']:r['curve']for r in apex_profiles()if r['id']in ['circle','periodic']});rows=[]
    def add(name,curve,crease,split,solid=False,apex=[0.,3.,2.],**extra):
        rows.append(dict(op='apex_splitting',id=name,curve=deepcopy(curve),apex=apex,solid=solid,delete_input=split,crease_splitting=crease,split_at_tangents=split,**extra))
    add('tangent_default',source['tangent'],False,True,set_split=False)
    for name in ['circle','periodic']:
        for below in [False,True]:
            for crease in [False,True]:
                for split in [False,True]:add(name+'_solid_'+str(int(below))+str(int(crease))+str(int(split)),source[name],crease,split,True,[5.,-4.,7.]if below else[0.,3.,2.])
    c1=dict(degree=3,knots=[0.]*4+[.5,.5]+[1.]*4,control_points=[dict(point=p,weight=1.)for p in [[3.,0.,0.],[4.,0.,1.],[2.,0.,2.],[5.,0.,3.],[2.,0.,4.],[3.,0.,5.]]])
    for crease in [False,True]:
        for split in [False,True]:add('cubic_c1_'+str(int(crease))+str(int(split)),c1,crease,split)
    for i,slope in enumerate([.0014,.0016,.0018,.0020,.0157,.0192]):
        curve=dict(degree=1,knots=[0.,0.,1.,2.,2.],control_points=[dict(point=p,weight=1.)for p in [[3.,0.,0.],[3.,0.,1.],[3.+slope,0.,2.]]])
        for crease,split in [(False,True),(True,False),(True,True)]:add('small_'+str(i)+'_'+str(int(crease))+str(int(split)),curve,crease,split)
    for name in ['tangent','closed']:
        for solid in [False,True]:add(name+'_mode_'+str(int(solid)),source[name],False,False,solid,seed_subd=True)
    return rows


def planes():
    source=profiles();source.update({r['id']:r['curve']for r in apex_profiles()if r['id']in ['circle','periodic']})
    return [dict(op='apex_plane_sdk',id='plane_'+name,curve=deepcopy(source[name]))for name in ['closed','circle','periodic']]


def validate(op):
    def same(a,b):
        if isinstance(b,dict):return isinstance(a,dict) and set(a)==set(b) and all(same(a[k],v)for k,v in b.items())
        if isinstance(b,list):return isinstance(a,list) and len(a)==len(b) and all(same(x,y)for x,y in zip(a,b))
        if isinstance(b,bool):return isinstance(a,bool) and a==b
        if isinstance(b,(int,float)):return not isinstance(a,bool) and isinstance(a,(int,float)) and a==b
        return type(a)==type(b) and a==b
    owned=next((r for r in pilot()+holdouts()+planes()if isinstance(op,dict) and r['id']==op.get('id')),None)
    if owned is None or not same(op,owned):raise ValueError('fixed owned apex splitting recipe required')
