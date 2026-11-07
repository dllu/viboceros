"""Owned command discovery for picked ArcBlend curve ends and options."""
def line(a,b):return dict(type='line',start=a,end=b)
def recipe(p,t0,q,t1,options=None):
 return dict(curves=[line([p[i]-t0[i]for i in range(3)],p),line(q,[q[i]+t1[i]for i in range(3)])],view='Top',actions=[dict(curve=0,pick=[p[i]-.05*t0[i]for i in range(3)]),dict(curve=1,pick=[q[i]+.05*t1[i]for i in range(3)])]+(options or[])+['Enter'])
BASE=([0.,0.,0.],[5.,0.,0.],[5.,4.,0.],[0.,5.,0.])
CASES={
 'arc_default':recipe(*BASE),
 'arc_s_shape':recipe([0.,0.,0.],[5.,0.,0.],[10.,3.,0.],[5.,0.,0.]),
 'arc_spatial':recipe([0.,0.,0.],[1.,2.,-1.],[7.,4.,3.],[-1.,1.,2.]),
 'arc_alternate':recipe(*BASE,options=['AlternateSolution']),
 'arc_radius_difference':recipe(*BASE,options=['RadiusDifference',0.]),
 'arc_unjoined':recipe(*BASE,options=['Trim=No','Join=No']),
}
PILOT=list(CASES)
HOLDOUTS={
 'arc_radius_zero':recipe(*BASE,options=['RadiusDifference',0.]),
 'arc_radius_positive':recipe(*BASE,options=['RadiusDifference',1.]),
 'arc_radius_negative':recipe(*BASE,options=['RadiusDifference',-1.]),
 'arc_trim':recipe(*BASE,options=['Trim=Yes']),
 'arc_join':recipe(*BASE,options=['Trim=Yes','Join=Yes']),
 'arc_ratio_direct':recipe(*BASE,options=[.25]),
 'arc_ratio_zero':recipe(*BASE,options=[0.]),
}
CASES.update(HOLDOUTS)
def validate(op):
 if not isinstance(op,dict)or set(op)!={'op','id','case'}or op.get('op')!='arc_blend_command'or not isinstance(op.get('case'),str)or op.get('case')not in CASES or op.get('id')!=op['case']:raise ValueError('invalid fixed ArcBlend command recipe')
def request(names=None):return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-9,angular=1e-6),operations=[dict(op='arc_blend_command',id=k,case=k)for k in(PILOT if names is None else names)])
