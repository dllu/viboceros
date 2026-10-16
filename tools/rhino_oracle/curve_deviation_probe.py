"""Owned CrvDeviation command histories and public curve-distance SDK calls."""
if globals().get('__package__'):
    from .geometry_validity_probe import validate as validate_source
else:
    from geometry_validity_probe import validate as validate_source


def validate(op):
    if (not isinstance(op,dict) or not {'op','id','curves','postselect'}<=set(op) or set(op)-{'op','id','curves','postselect','keep_marks','subcurves'} or op['op']!='curve_deviation'
        or not isinstance(op['curves'],list)or len(op['curves'])!=2 or type(op['postselect'])is not bool):raise ValueError('invalid curve deviation recipe')
    if 'keep_marks'in op and type(op['keep_marks'])is not bool:raise ValueError('invalid keep marks')
    if 'subcurves'in op:
        ranges=op['subcurves']
        if not op['postselect']or 'keep_marks'not in op or not isinstance(ranges,list)or len(ranges)!=2:raise ValueError('invalid subcurve input')
        for r in ranges:
            if r is not None and (not isinstance(r,list)or len(r)!=2 or any(type(t)not in(int,float)or not 0<=t<=1 for t in r)or r[0]==r[1]):raise ValueError('invalid subcurve range')
    for source in op['curves']:
        validate_source(dict(op='geometry_validity',id=op['id'],source=source))
        if source['type']not in ('line','arc','circle','ellipse','polyline','polycurve','nurbs'):raise ValueError('curve targets required')


def run(op,host,iterations,tolerance):
    validate(op)
    if iterations!=1:raise ValueError('curve deviation command capture requires one iteration')
    R=host['Rhino'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc;curves=[];ids=[];created=[]
    try:
        for source in op['curves']:curves.append(host['_object_source'](source,tolerance))
        values=G.Curve.GetDistancesBetweenCurves(curves[0],curves[1],tolerance['absolute'])
        ok=bool(values[0]);sdk=dict(created=ok,maximum=None,minimum=None)
        if ok:
            sdk['maximum']=dict(distance=float(values[1]),parameters=[float(values[2]),float(values[3])],points=[host['_xyz'](curves[0].PointAt(values[2])),host['_xyz'](curves[1].PointAt(values[3]))])
            sdk['minimum']=dict(distance=float(values[4]),parameters=[float(values[5]),float(values[6])],points=[host['_xyz'](curves[0].PointAt(values[5])),host['_xyz'](curves[1].PointAt(values[6]))])
        doc.Objects.UnselectAll()
        for c in curves:ids.append(doc.Objects.AddCurve(c))
        if any(i==host['System'].Guid.Empty for i in ids):raise ValueError('owned deviation sources failed')
        if not op['postselect']:
            for i in ids:doc.Objects.Select(i,True)
        marker='Viboceros deviation '+str(host['System'].Guid.NewGuid());R.RhinoApp.WriteLine(marker)
        macro='_-CrvDeviation'
        if 'subcurves'in op:
            for i,r in enumerate(op['subcurves']):
                if r is None:macro+=' _SelID '+str(ids[i])
                else:
                    picks=[curves[i].PointAt(curves[i].Domain.ParameterAt(t))for t in r]
                    macro+=' _SubCrv _SelID '+str(ids[i])+' '+' '.join(','.join(repr(x)for x in host['_xyz'](p))for p in picks)
        elif op['postselect']:macro+=' '+' '.join('_SelID '+str(i)for i in ids)
        macro+=(' _KeepMarks='+('Yes'if op['keep_marks']else'No')if 'keep_marks'in op else'')+' _Enter _Cancel'
        before=set(o.Id for o in doc.Objects)
        source_before=[host['_nurbs_curve_definition'](doc.Objects.FindId(i).Geometry)for i in ids]if 'subcurves'in op else None
        ran=R.RhinoApp.RunScript(macro,True)
        history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[-1].strip()
        if not history:raise ValueError('empty native deviation history')
        if 'Unknown command:'in history:raise ValueError('unknown command in deviation capture')
        created=[o.Id for o in doc.Objects if o.Id not in before]
        outputs=[]
        for i in created:
            obj=doc.Objects.FindId(i);g=obj.Geometry
            value=dict(type=str(g.GetType().Name),layer=doc.Layers[obj.Attributes.LayerIndex].Name,selected=bool(obj.IsSelected(False)),name=obj.Attributes.Name or None,color_source=str(obj.Attributes.ColorSource),color=[int(obj.Attributes.ObjectColor.R),int(obj.Attributes.ObjectColor.G),int(obj.Attributes.ObjectColor.B)])
            if isinstance(g,G.Point):value['point']=host['_xyz'](g.Location)
            elif isinstance(g,G.Curve):value.update(start=host['_xyz'](g.PointAtStart),end=host['_xyz'](g.PointAtEnd))
            elif isinstance(g,G.TextDot):value.update(point=host['_xyz'](g.Point),text=str(g.Text))
            outputs.append(value)
        result=dict(sdk=sdk,command_returned=bool(ran),command_history=history,inputs_retained=all(doc.Objects.FindId(i)is not None for i in ids),created_object_count=len(created))
        if 'keep_marks'in op:result['outputs']=outputs
        if 'subcurves'in op:
            source_after=[host['_nurbs_curve_definition'](doc.Objects.FindId(i).Geometry)if doc.Objects.FindId(i)is not None else None for i in ids]
            result.update(sdk_scope='whole_sources',source_ranges=[None if r is None else [float(curves[i].Domain.ParameterAt(t))for t in r]for i,r in enumerate(op['subcurves'])],inputs_unchanged=source_before==source_after,sources_before=source_before,sources_after=source_after)
        return result,0
    finally:
        R.RhinoApp.RunScript('!',False);doc.Objects.UnselectAll()
        for i in created+ids:doc.Objects.Delete(i,True)
        for c in curves:c.Dispose()
