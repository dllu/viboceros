"""Owned graph extraction workflow, attributes, selection and undo observations."""
if globals().get('__package__'):
    from .geometry_validity_probe import validate as validate_source
else:
    from geometry_validity_probe import validate as validate_source

def validate(op):
    if not isinstance(op,dict)or set(op)!={'op','id','sources','graphs_on','selected','scale','density','postselect','undo_redo'}or op['op']!='extract_curvature_graph':raise ValueError('invalid graph extraction recipe')
    if not isinstance(op['sources'],list)or not 1<=len(op['sources'])<=16:raise ValueError('invalid graph extraction sources')
    for source in op['sources']:
        validate_source(dict(op='geometry_validity',id=op['id'],source=source))
        if source['type']not in ('line','arc','circle','ellipse','nurbs','polyline','polycurve'):raise ValueError('curve sources required')
    for key in ('graphs_on','selected'):
        if not isinstance(op[key],list)or any(type(i)is not int or not 0<=i<len(op['sources'])for i in op[key])or len(set(op[key]))!=len(op[key]):raise ValueError('invalid source indices')
    if type(op['scale'])is not int or not 0<=op['scale']<=300 or type(op['density'])is not int or not 0<=op['density']<=10:raise ValueError('invalid graph settings')
    if type(op['postselect'])is not bool or type(op['undo_redo'])is not bool:raise ValueError('invalid graph workflow flags')

def run(op,host,iterations,tolerance):
    validate(op)
    if iterations!=1:raise ValueError('owned extraction requires one iteration')
    R=host['Rhino'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc;settings=R.ApplicationSettings.CurvatureGraphSettings
    old=(settings.HairScale,settings.HairDensity);old_layer=doc.Layers.CurrentLayerIndex
    ids=[];created=[];layers=[];geometries=[];group=None
    def snapshot():
        values=[]
        for obj in doc.Objects:
            if obj.Id in ids:continue
            g=obj.Geometry
            if not isinstance(g,G.Curve):continue
            a=obj.Attributes;ok,poly=g.TryGetPolyline()
            values.append(dict(type=str(g.GetType().Name),points=[host['_xyz'](p)for p in poly]if ok else None,selected=bool(obj.IsSelected(False)),layer=doc.Layers[a.LayerIndex].Name,name=a.Name or None,color_source=str(a.ColorSource),color=[int(a.ObjectColor.R),int(a.ObjectColor.G),int(a.ObjectColor.B)],group_count=int(a.GroupCount),user_text=a.GetUserString('graph-source'),geometry_user_text=g.GetUserString('graph-source')))
        return dict(outputs=values,sources_present=[doc.Objects.FindId(i)is not None for i in ids],sources_selected=[bool(doc.Objects.FindId(i).IsSelected(False))if doc.Objects.FindId(i)is not None else False for i in ids])
    try:
        for name in ('GraphSources','GraphOutput'):
            layer=R.DocObjects.Layer();layer.Name=name;index=doc.Layers.Add(layer);layers.append(index)
        for index,recipe in enumerate(op['sources']):
            g=host['_object_source'](recipe,tolerance);geometries.append(g)
            g.SetUserString('graph-source','geometry-value')
            attrs=R.DocObjects.ObjectAttributes();attrs.LayerIndex=layers[0];attrs.Name='source-'+str(index);attrs.ColorSource=R.DocObjects.ObjectColorSource.ColorFromObject;attrs.ObjectColor=host['System'].Drawing.Color.FromArgb(12,34,56);attrs.SetUserString('graph-source','attribute-value')
            ids.append(doc.Objects.AddCurve(g,attrs))
        doc.Layers.SetCurrentLayerIndex(layers[1],True)
        group=doc.Groups.Add('GraphSourcesGroup-'+op['id'],ids)
        before=[host['_nurbs_curve_definition'](doc.Objects.FindId(i).Geometry)for i in ids]
        settings.HairScale=op['scale'];settings.HairDensity=op['density']
        doc.Objects.UnselectAll()
        if op['graphs_on']:
            for i in op['graphs_on']:doc.Objects.Select(ids[i],True)
            R.RhinoApp.RunScript('_CurvatureGraph',False)
        doc.Objects.UnselectAll()
        if not op['postselect']:
            for i in op['selected']:doc.Objects.Select(ids[i],True)
        marker='Viboceros graph extraction '+str(host['System'].Guid.NewGuid());R.RhinoApp.WriteLine(marker)
        macro='_ExtractCurvatureGraph'
        if op['postselect']:macro+=' '+' '.join('_SelID '+str(ids[i])for i in op['selected'])+' _Enter'
        if not set(op['graphs_on']).intersection(op['selected']):macro+=' _Enter _Cancel'
        ran=R.RhinoApp.RunScript(macro,True);history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[-1].strip()
        if 'Unknown command:'in history:raise ValueError('unknown graph extraction command')
        created=[o.Id for o in doc.Objects if o.Id not in ids]
        result=snapshot();result.update(command_returned=bool(ran),command_history=history,inputs_unchanged=before==[host['_nurbs_curve_definition'](doc.Objects.FindId(i).Geometry)for i in ids])
        if op['undo_redo']and created:
            undone=R.RhinoApp.RunScript('_Undo',True);result['undo']=snapshot();result['undo']['command_returned']=bool(undone)
            result['undo']['command_history']=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[-1].strip()
            redone=R.RhinoApp.RunScript('_Redo',True);result['redo']=snapshot();result['redo']['command_returned']=bool(redone)
            result['redo']['command_history']=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[-1].strip()
        return result,0
    finally:
        R.RhinoApp.RunScript('! _CurvatureGraphOff',False);doc.Objects.UnselectAll()
        for obj in list(doc.Objects):
            if obj.Id in ids or obj.Id in created:doc.Objects.Delete(obj.Id,True)
        for g in geometries:g.Dispose()
        if group is not None and group>=0:doc.Groups.Delete(group)
        settings.HairScale,settings.HairDensity=old
        doc.Layers.SetCurrentLayerIndex(old_layer,True)
        for layer in reversed(layers):doc.Layers.Delete(layer,True)
