"""Owned graph command/extraction and independent public curvature samples."""
if globals().get('__package__'):
    from .geometry_validity_probe import validate as validate_source
else:
    from geometry_validity_probe import validate as validate_source

def validate(op):
    if set(op)!={'op','id','source','scale','density'} or op['op']!='curvature_graph':raise ValueError('invalid curvature graph recipe')
    if type(op['scale'])is not int or not 0<=op['scale']<=300:raise ValueError('invalid graph scale')
    if type(op['density'])is not int or not 0<=op['density']<=10:raise ValueError('invalid graph density')
    validate_source(dict(op='geometry_validity',id=op['id'],source=op['source']))
    if op['source']['type']not in ('line','arc','circle','ellipse','nurbs','polyline','polycurve'):raise ValueError('curve source required')

def run(op,host,iterations,tolerance):
    validate(op)
    if iterations!=1:raise ValueError('owned graph capture requires one iteration')
    R=host['Rhino'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc
    settings=R.ApplicationSettings.CurvatureGraphSettings
    old=(settings.HairScale,settings.HairDensity)
    source=host['_object_source'](op['source'],tolerance);ids=[];outputs=[]
    try:
        settings.HairScale=op['scale'];settings.HairDensity=op['density']
        key=doc.Objects.AddCurve(source);ids.append(key)
        before=host['_nurbs_curve_definition'](doc.Objects.FindId(key).Geometry)
        doc.Objects.UnselectAll();doc.Objects.Select(key,True)
        marker='Viboceros graph '+str(host['System'].Guid.NewGuid());R.RhinoApp.WriteLine(marker)
        ran=R.RhinoApp.RunScript('_CurvatureGraph _Enter',True)
        history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[-1].strip()
        snapshot=set(o.Id for o in doc.Objects)
        doc.Objects.UnselectAll();doc.Objects.Select(key,True)
        extracted=R.RhinoApp.RunScript('_ExtractCurvatureGraph _Enter',True)
        extraction_history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[-1].strip()
        if 'Unknown command:'in extraction_history:raise ValueError('unknown graph command')
        records=[]
        for obj in doc.Objects:
            if obj.Id in snapshot:continue
            outputs.append(obj.Id);g=obj.Geometry
            value=dict(type=str(g.GetType().Name))
            if isinstance(g,G.Curve):
                ok,poly=g.TryGetPolyline()
                value.update(start=host['_xyz'](g.PointAtStart),end=host['_xyz'](g.PointAtEnd))
                if ok:value['points']=[host['_xyz'](p)for p in poly]
            records.append(value)
        samples=[]
        for i in range(17):
            f=i/16.;t=source.Domain.ParameterAt(f)
            samples.append(dict(fraction=f,point=host['_xyz'](source.PointAt(t)),curvature=host['_xyz'](source.CurvatureAt(t))))
        after=host['_nurbs_curve_definition'](doc.Objects.FindId(key).Geometry)
        return dict(command_returned=bool(ran),command_history=history,extraction_returned=bool(extracted),extraction_history=extraction_history,outputs=records,samples=samples,inputs_unchanged=before==after,settings=dict(scale=settings.HairScale,density=settings.HairDensity)),0
    finally:
        R.RhinoApp.RunScript('_CurvatureGraphOff',False)
        for key in outputs+ids:doc.Objects.Delete(key,True)
        source.Dispose();settings.HairScale,settings.HairDensity=old
