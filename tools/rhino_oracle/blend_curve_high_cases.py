"""Prescribed higher-order sources, independent of any native output controls."""
def bezier(points,weights=None):
 degree=len(points)-1
 return dict(type='nurbs',degree=degree,control_points=[dict(point=p,weight=w)for p,w in zip(points,weights or[1.]*len(points))],knots=[0.]*(degree+1)+[1.]*(degree+1))
QUARTIC=bezier([[-8.,-4.,0.],[-6.,-3.,0.],[-3.,-2.,0.],[-2.,0.,0.],[0.,0.,0.]])
QUINTIC=bezier([[-10.,-4.,1.],[-8.,-2.,2.],[-6.,-3.,-1.],[-4.,-1.,1.],[-2.,0.,0.],[0.,0.,0.]])
RATIONAL=bezier([[-6.,-3.,2.],[-4.,-3.,1.],[-3.,0.,-1.],[0.,0.,0.]],[1.,2.,.5,1.])
def discovery(held,arc,cubic,line):
 cases={}
 for name,source in [('quartic',QUARTIC),('quintic',QUINTIC),('rational',RATIONAL)]:
  for order in [3,4]:
   cases['high_'+name+'_g'+str(order)]=held(order,order,curves=[source,line])
 cases['high_arc_g4']=held(4,4,curves=[arc,line],picks=[[-.25,-.006,0.],[10.25,3.,0.]])
 cases['high_cubic_g4']=held(4,4,curves=[cubic,line])
 cases['high_mixed_g3_g1']=held(3,1,curves=[QUARTIC,line])
 cases['high_mixed_g2_g4']=held(2,4,curves=[line,QUARTIC],picks=[[14.75,3.,0.],[-.25,0.,0.]])
 for value in cases.values():value['measure_jets']=True
 return cases

def holdouts(held,arc,line):
 cases={
  'high_shape_g3':held(3,3,extra=[dict(click=[1.3,0.,0.]),[2.5,0.,0.]]),
  'high_shape_g4':held(4,4,extra=[dict(click=[1.,0.,0.]),[2.5,0.,0.]]),
  'high_shape_quartic_g4':held(4,4,curves=[QUARTIC,line],extra=[dict(click=[1.,0.,0.]),[2.5,0.,0.]]),
  'high_slide_arc_g3':held(3,3,curves=[arc,line],picks=[[-.25,-.006,0.],[10.25,3.,0.]],extra=[dict(click=[0.,0.,0.]),[-3.,-1.,0.]]),
  'high_slide_quartic_g4':held(4,4,curves=[QUARTIC,line],extra=[dict(click=[0.,0.,0.]),[-2.,0.,0.]]),
  'high_flip_rational_g3':held(3,3,curves=[RATIONAL,line],extra=['Flip',dict(curve=0,pick=[-.25,0.,0.])]),
  'high_reverse_quintic_g4':held(4,4,curves=[QUINTIC,line],picks=[[-9.75,-3.8,1.],[14.75,3.,0.]]),
  'high_mixed_g0_g4':held(0,4,curves=[QUARTIC,line]),
  'high_join_g4':held(4,4,curves=[QUARTIC,line],extra=['Trim=Yes','Join=Yes']),
 }
 for value in cases.values():value['measure_jets']=True
 return cases
