"""Owned FitSrf command, SDK, metadata and history observations."""
import math


def validate(op):
    keys={'op','id','surface','degree_u','degree_v','fit_tolerance','delete_input','retrim','output_layer','preselect'}
    if (not isinstance(op,dict) or not keys.issubset(op) or set(op)-keys-{'construction'} or op['op']!='surface_fit_command'
            or not isinstance(op['id'],str) or not isinstance(op['surface'],dict)
            or op['output_layer']not in ['input','current']
            or op.get('construction','surface')not in ['surface','single_face_brep']):
        raise ValueError('invalid surface fit command operation')
    if any(type(op[k])is not bool for k in ['delete_input','retrim','preselect']):
        raise ValueError('invalid surface fit command switch')
    if any(type(op[k])is not int or not 1<=op[k]<=11 for k in ['degree_u','degree_v']):
        raise ValueError('invalid surface fit command degree')
    t=op['fit_tolerance']
    if type(t)not in (int,float) or math.isnan(t) or math.isinf(t) or t<0.:
        raise ValueError('invalid surface fit command tolerance')
    s=op['surface']
    if any(type(s.get('control_point_count_'+a))is not int or not 2<=s['control_point_count_'+a]<=64 for a in ['u','v']):
        raise ValueError('surface fit command source exceeds control limits')


def run(op,host,iterations,tolerance):
    validate(op)
    R,S=host['Rhino'],host['System'];doc=R.RhinoDoc.ActiveDoc;G=R.Geometry
    if iterations!=1 or R.Commands.Command.InCommand() or list(doc.Objects):
        raise ValueError('surface fitting requires one iteration in an empty owned idle document')
    old_layer=doc.Layers.CurrentLayerIndex;old_tolerance=doc.ModelAbsoluteTolerance
    layers=[];groups=[];ids=[]
    define=host['_nurbs_surface_definition']

    def snapshot():
        result=[]
        for obj in sorted(list(doc.Objects),key=lambda o:o.RuntimeSerialNumber):
            g,a=obj.Geometry,obj.Attributes
            if isinstance(g,G.Brep):
                surfaces=[f.UnderlyingSurface()for f in g.Faces];untrimmed=[bool(f.IsSurface)for f in g.Faces]
                brep=host['_interchange_brep_record'](g,include_samples=False)
            elif isinstance(g,G.Surface):
                surfaces=[g];untrimmed=[True];owned=g.ToBrep()
                try:brep=host['_interchange_brep_record'](owned,include_samples=False)
                finally:owned.Dispose()
            else:raise ValueError('unexpected FitSrf geometry: '+g.GetType().Name)
            result.append(dict(source=ids.index(obj.Id)if obj.Id in ids else None,
                selected=bool(obj.IsSelected(False)),name=a.Name,
                layer=layers.index(a.LayerIndex)if a.LayerIndex in layers else None,
                color=[int(a.ObjectColor.R),int(a.ObjectColor.G),int(a.ObjectColor.B)],color_source=str(a.ColorSource),
                groups=sorted(groups.index(i)for i in(a.GetGroupList()or[])if i in groups),
                attribute_text=a.GetUserString('Code'),geometry_text=g.GetUserString('Code'),
                valid=bool(g.IsValid),geometry_type=g.GetType().Name,untrimmed=untrimmed,
                definition=define(surfaces[0])if len(surfaces)==1 else None,
                definitions=[define(s)for s in surfaces],brep=brep))
        return result

    def invoke(macro,name):
        marker='Viboceros FitSrf '+str(S.Guid.NewGuid());events=[];errors=[]
        def ended(sender,event):
            try:
                item=dict(name=event.CommandEnglishName,result=str(event.CommandResult))
                if item['name']==name:item['objects']=snapshot()
                events.append(item)
            except Exception as error:errors.append(str(error))
        R.RhinoApp.WriteLine(marker);host['_record_progress'](op['id']+' '+macro)
        R.Commands.Command.EndCommand+=ended
        try:script_result=bool(R.RhinoApp.RunScript(macro,True))
        finally:R.Commands.Command.EndCommand-=ended
        active=bool(R.Commands.Command.InCommand())
        after=snapshot()
        history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[-1]
        if active:R.RhinoApp.RunScript('!',False)
        if errors:raise ValueError('surface fit observer failed: '+str(errors))
        completed=[e for e in events if e['name']==name]
        return dict(script_result=script_result,events=events,completed=len(completed),
                    success=len(completed)==1 and completed[0]['result']=='Success'and not active,
                    active=active,after=after,after_cancel=snapshot(),history=history,macro=macro)

    try:
        doc.ModelAbsoluteTolerance=tolerance['absolute']
        for label in ['input','current']:
            layer=R.DocObjects.Layer();layer.Name='FitSrf '+label+' '+str(S.Guid.NewGuid())
            try:layers.append(doc.Layers.Add(layer))
            finally:layer.Dispose()
        source=host['_nurbs_surface_from_definition'](op['surface']);attrs=R.DocObjects.ObjectAttributes()
        try:
            attrs.Name='source';attrs.LayerIndex=layers[0]
            attrs.ObjectColor=S.Drawing.Color.FromArgb(20,40,60);attrs.ColorSource=R.DocObjects.ObjectColorSource.ColorFromObject
            attrs.SetUserString('Code','attribute');source.SetUserString('Code','geometry')
            if op.get('construction','surface')=='single_face_brep':
                brep=G.Brep.CreateFromSurface(source)
                try:
                    if brep.Faces.Count!=1:raise ValueError('explicit single-face construction failed')
                    ids.append(doc.Objects.AddBrep(brep,attrs,None,False,False))
                finally:brep.Dispose()
            else:ids.append(doc.Objects.AddSurface(source,attrs))
            if ids[-1]==S.Guid.Empty:raise ValueError('FitSrf source insertion failed')
            fitted=source.Fit(op['degree_u'],op['degree_v'],op['fit_tolerance']or doc.ModelAbsoluteTolerance)
            candidate=None
            try:
                candidate=fitted.ToNurbsSurface()if fitted is not None else None
                sdk=define(candidate)if candidate is not None else None
            finally:
                if candidate is not None and candidate is not fitted:candidate.Dispose()
                if fitted is not None:fitted.Dispose()
        finally:source.Dispose();attrs.Dispose()
        groups.append(doc.Groups.Add('FitSrf '+str(S.Guid.NewGuid()),ids))
        doc.Layers.SetCurrentLayerIndex(layers[1],True);doc.ClearUndoRecords(True)
        if op['preselect']:doc.Objects.Select(ids[0])
        before=snapshot()
        face_sdk=[]
        geometry=doc.Objects.FindId(ids[0]).Geometry
        for face in (geometry.Faces if isinstance(geometry,G.Brep)else[geometry]):
            fitted=face.Fit(op['degree_u'],op['degree_v'],op['fit_tolerance']or doc.ModelAbsoluteTolerance)
            candidate=None
            try:
                candidate=fitted.ToNurbsSurface()if fitted is not None else None
                face_sdk.append(define(candidate)if candidate is not None else None)
            finally:
                if candidate is not None and candidate is not fitted:candidate.Dispose()
                if fitted is not None:fitted.Dispose()
        macro='_-FitSrf '
        if not op['preselect']:macro+='_SelID '+str(ids[0])+' _Enter '
        macro+='_UDegree='+str(op['degree_u'])+' _VDegree='+str(op['degree_v'])
        macro+=' _DeleteInput='+('Yes'if op['delete_input']else'No')+' _ReTrim='+('Yes'if op['retrim']else'No')
        macro+=' _OutputLayer=_'+('Current'if op['output_layer']=='current'else'Input')+' '+str(op['fit_tolerance'])
        command=invoke(macro,'FitSrf')
        return dict(before=before,sdk=sdk,face_sdk=face_sdk,document_tolerance=doc.ModelAbsoluteTolerance,command=command,
                    undo=invoke('_Undo','Undo'),redo=invoke('_Redo','Redo')),0
    finally:
        if R.Commands.Command.InCommand():R.RhinoApp.RunScript('!',False)
        doc.Objects.UnselectAll()
        for obj in list(doc.Objects):doc.Objects.Delete(obj.Id,True)
        for group in groups:doc.Groups.Delete(group)
        doc.Layers.SetCurrentLayerIndex(old_layer,True)
        for layer in reversed(layers):doc.Layers.Delete(layer,True)
        doc.ModelAbsoluteTolerance=old_tolerance
