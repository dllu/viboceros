"""Owned profiles for explicit extrusion splits and application preferences."""
from copy import deepcopy
if __package__:
    from .revolve_subd_cases import profiles
    from .extrude_apex_subd_cases import holdouts as apex_profiles
else:
    from revolve_subd_cases import profiles
    from extrude_apex_subd_cases import holdouts as apex_profiles


def pilot():
    curves=profiles();rows=[]
    def add(name,profile,crease,split,use=False,solid=False,direction=None):
        rows.append(dict(op='extrusion_splitting',id=name,curve=deepcopy(curves[profile]),direction=direction or [0.,3.,0.],both_sides=False,solid=solid,delete_input=bool(split),crease_splitting=crease,split_at_tangents=split,use_extrusions=use))
    add('tangent_default','tangent',False,None)
    for name in ['smooth','tangent','corner','polyline','rational','closed']:
        for crease in [False,True]:
            for split in [False,True]:add(name+'_'+str(int(crease))+str(int(split)),name,crease,split)
            add(name+'_typed_'+str(int(crease)),name,crease,None,True)
    for crease in [False,True]:
        for split in [False,True]:add('closed_solid_'+str(int(crease))+str(int(split)),'closed',crease,split,solid=True)
        add('closed_solid_typed_'+str(int(crease)),'closed',crease,None,True,True)
    for name in ['tangent','corner','closed']:
        for crease in [False,True]:
            for split in [False,True]:add(name+'_oblique_'+str(int(crease))+str(int(split)),name,crease,split,solid=name=='closed',direction=[1.,3.,2.])
    return rows


def holdouts():
    curves=profiles();curves.update({r['id']:r['curve']for r in apex_profiles()if r['id']in ['circle','periodic']});rows=[]
    def add(name,curve,crease,split,direction=[0.,3.,0.],solid=False,both=False,use=False,**extra):
        rows.append(dict(op='extrusion_splitting',id=name,curve=deepcopy(curve),direction=direction,both_sides=both,solid=solid,delete_input=bool(split),crease_splitting=crease,split_at_tangents=split,use_extrusions=use,**extra))
    for name in ['circle','periodic']:
        for negative in [False,True]:
            for crease in [False,True]:
                for split in [False,True]:add(name+'_solid_'+str(int(negative))+str(int(crease))+str(int(split)),curves[name],crease,split,[0.,-3.,0.]if negative else[0.,3.,0.],True)
    for oblique in [False,True]:
        for crease in [False,True]:
            for split in [False,True]:add('closed_reverse_'+str(int(oblique))+str(int(crease))+str(int(split)),curves['closed'],crease,split,[1.,-3.,2.]if oblique else[0.,-3.,0.],True,True)
    for i,slope in enumerate([.0016,.0018,.0157,.0192]):
        curve=dict(degree=1,knots=[0.,0.,1.,2.,2.],control_points=[dict(point=p,weight=1.)for p in [[3.,0.,0.],[3.,0.,1.],[3.+slope,0.,2.]]])
        for crease,split in [(False,True),(True,False),(True,True)]:add('small_'+str(i)+'_'+str(int(crease))+str(int(split)),curve,crease,split)
    rational=deepcopy(curves['rational']);rational['knots']=[100.+4.*t for t in rational['knots']]
    c1=dict(degree=3,knots=[0.]*4+[.5,.5]+[1.]*4,control_points=[dict(point=p,weight=1.)for p in [[3.,0.,0.],[4.,0.,1.],[2.,0.,2.],[5.,0.,3.],[2.,0.,4.],[3.,0.,5.]]])
    for crease in [False,True]:
        for split in [False,True]:
            add('rational_domain_'+str(int(crease))+str(int(split)),rational,crease,split,[0.,-3.,0.])
            add('cubic_c1_'+str(int(crease))+str(int(split)),c1,crease,split,both=True)
    for name in ['smooth','tangent','corner','polyline','rational','closed']:
        for crease in [False,True]:add(name+'_oblique_typed_'+str(int(crease)),curves[name],crease,None,[1.,3.,2.],solid=name=='closed',use=True)
    for name in ['tangent','closed']:add(name+'_mode',curves[name],False,False,solid=True,both=True,seed_subd=True)
    return rows


def confirmations():
    curves=profiles();rows=[]
    for name in ['tangent','closed']:
        for oblique in [False,True]:
            for seed in [False,True]:
                rows.append(dict(op='extrusion_splitting',id=name+'_hidden_'+str(int(oblique))+str(int(seed)),curve=deepcopy(curves[name]),direction=[1.,3.,2.]if oblique else[0.,3.,0.],both_sides=False,solid=name=='closed',delete_input=False,crease_splitting=False,split_at_tangents=None,use_extrusions=True,seed_split=seed))
        rows.append(dict(op='extrusion_splitting',id=name+'_hidden_mode',curve=deepcopy(curves[name]),direction=[1.,3.,2.],both_sides=False,solid=True,delete_input=False,crease_splitting=False,split_at_tangents=None,use_extrusions=True,seed_subd=True))
    return rows


def cancellations():
    curve=profiles()['tangent']
    return [dict(op='extrusion_splitting',id='cancel_split_'+str(int(seed)),curve=deepcopy(curve),direction=[1.,3.,2.],both_sides=False,solid=False,delete_input=False,crease_splitting=False,split_at_tangents=None,use_extrusions=True,seed_split=seed,seed_cancel=True)for seed in [False,True]]


def curved():
    rows=[]
    closed=dict(degree=2,knots=[0.]*3+[1.,1.,2.,2.,3.,3.]+[4.]*3,control_points=[dict(point=p,weight=w)for p,w in zip([[2.,0.,0.],[2.,0.,2.],[0.,0.,2.],[-2.,0.,2.],[-2.,0.,0.],[-2.,0.,-1.5],[0.,0.,-1.5],[2.,0.,-1.5],[2.,0.,0.]],[1.,.6,1.1,.9,.75,1.2,1.5,.8,1.])])
    nonplanar=dict(degree=3,knots=[0.]*4+[.5,.5]+[1.]*4,control_points=[dict(point=p,weight=1.)for p in [[3.,0.,0.],[4.,0.,1.],[2.,0.,2.],[5.,1.,3.],[2.,-1.,4.],[3.,0.,5.]]])
    def add(name,curve,direction,crease,split,solid,use=False,both=False):
        rows.append(dict(op='extrusion_splitting',id=name,curve=deepcopy(curve),direction=direction,both_sides=both,solid=solid,delete_input=bool(split),crease_splitting=crease,split_at_tangents=split,use_extrusions=use))
    for sign in [-1.,1.]:
        for crease in [False,True]:
            for split in [False,True]:
                suffix=('negative'if sign<0 else'positive')+'_'+str(int(crease))+str(int(split))
                add('closed_rational_'+suffix,closed,[0.,3.*sign,0.],crease,split,True,both=sign<0)
                add('nonplanar_'+suffix,nonplanar,[1.,3.*sign,2.],crease,split,False)
    for crease in [False,True]:
        add('closed_rational_typed_'+str(int(crease)),closed,[0.,3.,0.],crease,None,True,True)
        for split in [False,True]:add('closed_rational_oblique_'+str(int(crease))+str(int(split)),closed,[1.,3.,2.],crease,split,True)
    return rows


def mixed():
    source=curved()[0]['curve'];curve=deepcopy(source);curve['control_points'][5]['point']=[-1.,0.,-1.5]
    small=dict(degree=1,knots=[0.,0.,1.,2.,3.,4.,5.,5.],control_points=[dict(point=p,weight=1.)for p in [[0.,0.,0.],[1.,0.,0.],[2.,0.,.002],[2.,0.,2.],[0.,0.,2.],[0.,0.,0.]]])
    rows=[]
    for name,profile in [('mixed_rational',curve),('small_closed',small)]:
        for sign in [-1.,1.]:
            for crease in [False,True]:
                for split in [False,True]:
                    rows.append(dict(op='extrusion_splitting',id=name+('_negative_'if sign<0 else'_positive_')+str(int(crease))+str(int(split)),curve=deepcopy(profile),direction=[0.,3.*sign,0.],both_sides=False,solid=True,delete_input=split,crease_splitting=crease,split_at_tangents=split,use_extrusions=False))
    return rows


def validate(op):
    def same(a,b):
        if isinstance(b,dict):return isinstance(a,dict) and set(a)==set(b) and all(same(a[k],v)for k,v in b.items())
        if isinstance(b,list):return isinstance(a,list) and len(a)==len(b) and all(same(x,y)for x,y in zip(a,b))
        if isinstance(b,bool):return isinstance(a,bool) and a==b
        if isinstance(b,(int,float)):return not isinstance(a,bool) and isinstance(a,(int,float)) and a==b
        return type(a)==type(b) and a==b
    owned=next((r for r in pilot()+holdouts()+confirmations()+cancellations()+curved()+mixed()if isinstance(op,dict) and r['id']==op.get('id')),None)
    if owned is None or not same(op,owned):raise ValueError('fixed owned extrusion splitting recipe required')
