"""Public SDK contacts beside acknowledged native Line command observations."""
if __package__:from .line_tangent_arcs import CASES,validate
else:from line_tangent_arcs import CASES,validate

def contacts(host,source,target,station,tolerance,fractions=None,subdomain=None,refine=False):
 R=host['Rhino'];curve=host['_object_source'](source,tolerance);result=[]
 if source.get('domain'):curve.Domain=R.Geometry.Interval(*source['domain'])
 if refine:
  nurbs=curve.ToNurbsCurve();curve.Dispose();curve=nurbs
  curve.Knots.InsertKnot(curve.Domain.Mid)
 try:
  for representation in ['analytic','nurbs']:
   c=curve if representation=='analytic'else curve.ToNurbsCurve()
   try:
    ok,seed=c.ClosestPoint(R.Geometry.Point3d(*station))
    if not ok:raise ValueError('SDK contact station unavailable')
    rows=[]
    domain=c.Domain if subdomain is None else R.Geometry.Interval(c.Domain.ParameterAt(subdomain[0]),c.Domain.ParameterAt(subdomain[1]))
    for t in [seed]+[domain.ParameterAt(f)for f in ([0.,.1,.25,.4,.5,.6,.75,.9,1.]if fractions is None else fractions)]:
     ok,parameter=c.GetLocalTangentPoint(R.Geometry.Point3d(*target),t)if subdomain is None else c.GetLocalTangentPoint(R.Geometry.Point3d(*target),t,domain)
     row=dict(seed=float(t),success=bool(ok),raw_parameter=float(parameter))
     if ok:row.update(parameter=float(parameter),point=host['_xyz'](c.PointAt(parameter)),tangent=host['_xyz'](c.TangentAt(parameter)))
     rows.append(row)
    result.append(dict(representation=representation,native_type=c.GetType().Name,domain=[c.Domain.T0,c.Domain.T1],search_domain=[domain.T0,domain.T1],queries=rows))
   finally:
    if c is not curve:c.Dispose()
 finally:curve.Dispose()
 return result

def run(op,host,iterations,tolerance):
 validate(op)
 if iterations!=1:raise ValueError('one owned tangent observation required')
 from line_scene_probe import run_case
 case=CASES[op['case']]
 value,elapsed=run_case(op,host,iterations,tolerance,case,'Line',False)
 click=value['input_trace'][0]['clicks'][0]
 value['prescribed_case']=case
 value['contact_sdk']=contacts(host,case['curves'][0],case['actions'][-1],click['sight_point'],tolerance)
 return value,elapsed
