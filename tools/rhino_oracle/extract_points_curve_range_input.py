"""Bounded whole-curve and temporary-portion selection sequences."""
def run(doc, ids, case, host, options):
    parameters,points=[],[]
    for source,start,end in case['ranges']:
        curve=doc.Objects.FindId(ids[source]).Geometry
        interval=[curve.Domain.ParameterAt(f) for f in [start,end]]
        parameters.append([source]+interval)
        points.append([source]+[host['_xyz'](curve.PointAt(t)) for t in interval])
    inputs=case.get('input_order') or ([['whole',i] for i in case['whole']]+
        [['range',i] for i in range(len(parameters))])
    script='_-ExtractPt '+' '.join(options)+' '
    for kind,index in inputs:
        if kind=='whole':script+=' _SelID '+str(ids[index])
        elif kind=='range':
            source=parameters[index][0]
            tokens=['w'+','.join(repr(float(x)) for x in p) for p in points[index][1:]]
            script+=' _SubCrv _SelID '+str(ids[source])+' '+' '.join(tokens)
        else:raise ValueError('unknown owned ExtractPt input kind')
    script+=(' _SelPt' if case.get('range_grips') else '')+' _Enter _Cancel _Cancel'
    doc.Objects.UnselectAll()
    return host['Rhino'].RhinoApp.RunScript(script,True),dict(
        parameters=parameters,points=points,script=script,inputs=inputs,world_coordinates=True)
