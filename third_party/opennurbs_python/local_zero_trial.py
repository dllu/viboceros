# Research adaptation of public ON_LocalZero1::NewtonRaphson from
# opennurbs_optimize.cpp at 23fc677ba06e49212296ca75fab7fb6c2851b4ce.
# Portions Copyright (c) 1993-2022 Robert McNeel & Associates. All rights reserved.
# Modified Python experiment, not an upstream OpenNURBS component.
# MIT/developer-tools license: third_party/opennurbs_python/LICENSE.
# Preserve the public routine's output-assignment edge case for investigation.
def root(fun,a,b,ftol=0.,ttol=1e-8,maxit=128):
 f0,_=fun(a);f1,_=fun(b)
 if abs(f0)<=ftol and abs(f0)<=abs(f1):return a,'end'
 if abs(f1)<=ftol:return b,'end'
 if f0>0:a,b,f0,f1=b,a,f1,f0
 s=.5*(a+b);f,d=fun(s)
 if abs(f)<=ftol:return s,'mid'
 if f1<=0:return None,'bracket'
 ds=abs(b-a);prev=0.
 for i in range(maxit):
  if (f+(a-s)*d)*(f+(b-s)*d)>0 or abs(2*f)>abs(prev*d):
   prev=ds;ds=.5*(b-a);s=a+ds
   if s==a:return (b if abs(f1)<abs(f0)else s),'round_bisect'
  else:
   prev=ds;ds=-f/d;old=s;s+=ds
   if s==old:
    if abs(f0)<abs(f):f,s=f0,a
    if abs(f1)<abs(f):s=b
    return s,'round_newton'
  f,d=fun(s)
  if abs(f)<=ftol:
   out=None
   if abs(f0)<abs(f):f,out=f0,a
   if abs(f1)<abs(f):out=b
   return out,'unassigned'
  if f<0:f0,a=f,s
  else:f1,b=f,s
  if abs(b-a)<=ttol:return (a if abs(f0)<=abs(f1)else b),'interval'
 return (a if abs(f0)<=abs(f1)else b),'maxit'
