"""Controlled search-rule alternatives using only the public blend constructor."""
def observe(recipe,host):
 G=host['Rhino'].Geometry;p,q=recipe['start'],recipe['end'];t0,t1=recipe['start_direction'],recipe['end_direction'];target=recipe['search_target']
 def radii(a,ratio=None):
  c=G.Curve.CreateArcBlend(G.Point3d(*p),G.Vector3d(*t0),G.Point3d(*q),G.Vector3d(*t1),a/(1.-a)if ratio is None else ratio)
  if c is None:raise ValueError('public search-rule trial failed')
  try:return c.SegmentCurve(0).Arc.Radius-c.SegmentCurve(1).Arc.Radius
  finally:c.Dispose()
 output=[]
 for exact in [False,True]:
  for branch_equal in [False,True]:
   for best_equal in [False,True]:
    for difference_mid in [False,True]:
     for limit in [32,64]:
      lo,hi=1./101.,100./101.;lv=radii(lo,.01 if exact else None);hv=radii(hi,100. if exact else None)
      best=(lo,lv)if abs(lv-target)<abs(hv-target)else(hi,hv)
      for i in range(limit):
       a=lo+.5*(hi-lo)if difference_mid else .5*(lo+hi)
       if a==lo or a==hi:break
       f=radii(a)
       if abs(f-target)<abs(best[1]-target)or best_equal and abs(f-target)==abs(best[1]-target):best=a,f
       if abs(f-target)<=1./4294967296.:best=a,f;break
       if abs(lv-target)<abs(hv-target)or branch_equal and abs(lv-target)==abs(hv-target):hi,hv=a,f
       else:lo,lv=a,f
      output.append(dict(exact_bounds=exact,branch_equal=branch_equal,best_equal=best_equal,difference_mid=difference_mid,limit=limit,fraction=best[0],radius_difference=best[1],iterations=i+1))
 return output
