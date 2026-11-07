"""Read public ArcCurve radii and independent native evaluation samples."""
def observe(curve,host):
 G=host['Rhino'].Geometry
 if not isinstance(curve,G.PolyCurve):return {}
 segments=[]
 for i in range(curve.SegmentCount):
  c=curve.SegmentCurve(i)
  if not isinstance(c,G.ArcCurve):return {}
  a=c.Arc
  segments.append(dict(radius=a.Radius,center=host['_xyz'](a.Center),angles=[a.AngleDomain.T0,a.AngleDomain.T1],
   curve=host['_nurbs_curve_definition'](c),native_valid=bool(c.IsValid),
   samples=[dict(parameter=c.Domain.ParameterAt(j/32.),point=host['_xyz'](c.PointAt(c.Domain.ParameterAt(j/32.))),tangent=host['_xyz'](c.TangentAt(c.Domain.ParameterAt(j/32.))))for j in range(33)]))
 return dict(arc_segments=segments,samples=[dict(parameter=curve.Domain.ParameterAt(j/32.),point=host['_xyz'](curve.PointAt(curve.Domain.ParameterAt(j/32.))),tangent=host['_xyz'](curve.TangentAt(curve.Domain.ParameterAt(j/32.))))for j in range(33)])
