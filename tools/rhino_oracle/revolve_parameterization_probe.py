"""Public SDK and exact-command controls for revolution parameter domains."""
if __package__:
    from .revolve_deformable_cases import cases
else:
    from revolve_deformable_cases import cases


def validate(op):
    if not isinstance(op,dict) or set(op)!=set(['op','id','case']) or op['op']!='revolve_parameterization' or op['id']!=op['case'] or op['case'] not in ['line','rational','spatial','axis_tips']:
        raise ValueError('fixed owned revolution parameter query required')


def run(op,host,iterations,tolerance):
    validate(op)
    R=host['Rhino'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc
    if iterations!=1 or list(doc.Objects) or R.Commands.Command.InCommand():raise ValueError('empty owned idle document required')
    case=next(row for row in cases() if row['id']==op['case']+'_8_360')
    curve=host['_nurbs_curve_from_definition'](case['curve'])
    axis=G.Line(G.Point3d(*case['origin']),G.Point3d(*case['axis_end']))
    def sdk(c):
        rev=G.RevSurface.Create(c,axis,0.,6.283185307179586)
        if rev is None:raise ValueError('public revolution creation failed')
        try:return dict(size=list(rev.GetSurfaceSize()),surface=host['_nurbs_surface_definition'](rev),profile=host['_nurbs_curve_definition'](c),length=float(c.GetLength()))
        finally:rev.Dispose()
    try:
        original=sdk(curve)
        curve.Domain=G.Interval(0.,curve.GetLength())
        rescaled=sdk(curve)
        source=doc.Objects.AddCurve(curve);doc.Objects.Select(source);doc.ClearUndoRecords(True)
        p=lambda a:'w'+','.join('%.17g'%v for v in a)
        macro='_-Revolve %s %s _Output=_Surface _Deformable=_No _DeleteInput=_No _SplitAtTangents=_No _AskForStartAngle=_Yes 30 360'%(p(case['origin']),p(case['axis_end']))
        marker='Owned exact domain '+str(host['System'].Guid.NewGuid());R.RhinoApp.WriteLine(marker)
        success=R.RhinoApp.RunScript(macro,True)
        out=[]
        for obj in doc.Objects:
            if obj.Id==source:continue
            g=obj.Geometry
            if isinstance(g,G.Brep):out.extend(host['_nurbs_surface_definition'](f.UnderlyingSurface())for f in g.Faces)
            else:out.append(host['_nurbs_surface_definition'](g))
        return dict(original=original,rescaled=rescaled,exact_success=success,exact=out,history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip()),0
    finally:
        curve.Dispose();R.RhinoApp.RunScript('!',False)
        for obj in list(doc.Objects):doc.Objects.Delete(obj.Id,True)
        doc.ClearUndoRecords(True)
