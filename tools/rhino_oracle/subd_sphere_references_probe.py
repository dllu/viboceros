"""Fixed public sphere tangency, guided bases, fitting, sources and history."""
from copy import deepcopy
LINES=[dict(type='line',start=[0.,0.,0.],end=[12.,0.,0.]),dict(type='line',start=[0.,0.,0.],end=[0.,12.,0.]),dict(type='line',start=[0.,10.,0.],end=[10.,0.,0.])]
CIRCLE=dict(type='circle',center=[0.,0.,0.],radius=2.,x_axis=[1.,0.,0.],normal=[0.,0.,1.])
CARDINAL=[[2.,0.,0.],[0.,2.,0.],[-2.,0.,0.],[0.,-2.,0.]]
def scene(actions,style='Quad',**kw):return dict(actions=['Style='+style,'Subdivisions=1']+actions,**kw)
def pick(i,p):return dict(curve=i,pick=p)
SCENES={
 'sphere_tangent_points':scene(['Tangent','Point',[3.,0.,0.],'Point',[0.,3.,0.],'Point',[-3.,0.,0.]]),
 'sphere_tangent_spatial':scene(['Tangent','Point',[1.,2.,3.],'Point',[4.,6.,3.],'Point',[0.,5.,5.]]),
 'sphere_tangent_two':scene(['Tangent',pick(0,[3.,0.,0.]),pick(1,[0.,4.,0.]),'Enter'],curves=LINES[:2],live=True),
 'sphere_tangent_radius':scene(['Tangent',pick(0,[3.,0.,0.]),3.,pick(1,[0.,4.,0.])],curves=LINES[:2],live=True),
 'sphere_tangent_late_radius':scene(['Tangent',pick(0,[3.,0.,0.]),pick(1,[0.,4.,0.]),'Radius',3.],curves=LINES[:2],live=True),
 'sphere_tangent_from_first':scene(['Tangent',pick(0,[3.,0.,0.]),'FromFirstPoint',pick(1,[0.,4.,0.])],curves=LINES[:2],live=True),
 'sphere_tangent_three':scene(['Tangent',pick(0,[3.,0.,0.]),pick(1,[0.,4.,0.]),pick(2,[5.,5.,0.])],curves=LINES,live=True),
 'sphere_tangent_circle_line':scene(['Tangent',pick(0,[1.2,1.6,0.]),pick(1,[1.,8.,0.]),'Enter'],curves=[CIRCLE,dict(type='line',start=[-10.,8.,0.],end=[14.,8.,0.])],live=True),
 'sphere_tangent_triangle_style':scene(['Tangent',pick(0,[3.,0.,0.]),3.,pick(1,[0.,4.,0.])],style='Triangle',curves=LINES[:2],live=True),
 'sphere_tangent_cancel':scene(['Tangent',pick(0,[3.,0.,0.]),'Cancel'],curves=LINES[:2],live=True,cancel=True),
 'sphere_fit_preselected':scene(['FitPoints'],points=CARDINAL[:3],selected=[0,1,2]),
 'sphere_fit_postselected':scene(['FitPoints','SelAll','Enter'],points=CARDINAL+[[0.,0.,2.]]),
 'sphere_fit_partial':scene(['FitPoints','SelAll','Enter'],points=CARDINAL+[[0.,0.,2.]],selected=[0,2]),
 'sphere_fit_noisy':scene(['FitPoints'],points=[[2.,0.,0.],[0.,2.,0.],[-2.,0.,0.],[0.,-2.,0.],[0.,0.,2.02],[0.,0.,-2.]],selected=[0,1,2,3,4,5]),
 'sphere_fit_grips':scene(['FitPoints'],controls=CARDINAL,weights=[1.,2.,.5,1.],selected_grips=[0,1,2,3]),
 'sphere_fit_mixed':scene(['FitPoints'],controls=CARDINAL,selected_grips=[0],points=CARDINAL[1:3],selected=[0,1]),
 'sphere_fit_grips_postselected':scene(['FitPoints','SelAll','Enter'],controls=CARDINAL),
 'sphere_fit_collinear':scene(['FitPoints'],points=[[0.,0.,0.],[1.,0.,0.],[2.,0.,0.]],selected=[0,1,2],no_output=True),
 'sphere_fit_collinear_postselected':scene(['FitPoints','SelAll','Enter'],points=[[0.,0.,0.],[1.,0.,0.],[2.,0.,0.]],no_output=True),
 'sphere_fit_insufficient':scene(['FitPoints','SelAll','Enter','Cancel'],points=CARDINAL[:2],cancel=True),
 'sphere_fit_cancel':scene(['FitPoints','Cancel'],points=CARDINAL,cancel=True),
 'sphere_around_z':scene(['AroundCurve',{'curve':0},[0.,0.,2.],3.],curves=[dict(type='line',start=[0.,0.,0.],end=[0.,0.,10.])],live=True),
 'sphere_around_x':scene(['AroundCurve',{'curve':0},[2.,0.,0.],[2.,3.,4.]],curves=[dict(type='line',start=[0.,0.,0.],end=[10.,0.,0.])],live=True),
 'sphere_around_preselected':scene(['AroundCurve',[0.,0.,2.],3.],curves=[dict(type='line',start=[0.,0.,0.],end=[0.,0.,10.])],selected_curves=[0],live=True),
 'sphere_around_circle':scene(['AroundCurve',{'curve':0},[0.,2.,0.],3.],curves=[CIRCLE],live=True),
}
def request():return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=[dict(op='subd_sphere_references',id=k,case=k)for k in SCENES])
def validate(op):
 try:valid=isinstance(op,dict)and set(op)=={'op','id','case'}and op['op']=='subd_sphere_references'and op['case']in SCENES and op['id']==op['case']
 except TypeError:valid=False
 if not valid:raise ValueError('fixed SubDSphere reference recipe required')
def run(op,host,iterations,tolerance):
 validate(op)
 from subd_cylinder_references_probe import run_scene
 return run_scene(op,host,iterations,tolerance,'SubDSphere',SCENES[op['case']])
