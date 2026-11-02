"""Owned public Loft command, explicit crease/tangent policies, and undo states."""


def validate(op):
    required={'op','id','curves','style','closed','crease_splitting','split_at_tangents','preselect'}
    if (not isinstance(op,dict) or not required<=set(op) or set(op)-required-{'natural','inspection','explicit_options','orientation_inspection','orientation_witness'}
            or op['op']!='loft_splitting' or op['style']not in ['normal','loose','tight','straight','uniform']
            or any(type(op[k])is not bool for k in ['closed','crease_splitting','split_at_tangents','preselect'])
            or any(type(op.get(k,False))is not bool for k in ['natural','inspection','explicit_options','orientation_inspection','orientation_witness'])
            or not isinstance(op['curves'],list) or not (3 if op['closed']else 2)<=len(op['curves'])<=32):
        raise ValueError('invalid loft splitting request')
    try:
        from .curve_definition_validation import validate_curve_definition
    except (ImportError,ValueError,SystemError):
        from curve_definition_validation import validate_curve_definition
    for curve in op['curves']:validate_curve_definition(curve)


def run(op,host,iterations,tolerance):
    validate(op)
    try:
        from .join_probe import observe_command
    except (ImportError,ValueError,SystemError):
        from join_probe import observe_command
    R,S=host['Rhino'],host['System'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc
    if iterations!=1 or list(doc.Objects) or doc.UndoRecordingIsActive:
        raise ValueError('loft splitting requires an empty owned idle document')
    saved=(doc.ModelAbsoluteTolerance,doc.ModelRelativeTolerance,doc.ModelAngleToleranceRadians)
    owned=[];ids=[];crcs=[]
    def snapshot():
        records=[]
        for obj in sorted(list(doc.Objects),key=lambda obj:obj.RuntimeSerialNumber):
            if obj.Id in ids:
                i=ids.index(obj.Id)
                records.append(dict(source=i,unchanged=int(obj.Geometry.DataCRC(0))==crcs[i],selected=bool(obj.IsSelected(False))))
                continue
            b=obj.Geometry
            if not isinstance(b,G.Brep) or not b.IsValid:raise ValueError('invalid Loft result')
            faces=[]
            box=b.GetBoundingBox(True)
            for face in b.Faces:
                u,v=face.Domain(0),face.Domain(1)
                faces.append(dict(domain_u=[u.T0,u.T1],domain_v=[v.T0,v.T1],reversed=bool(face.OrientationIsReversed),
                    samples=[host['_xyz'](face.PointAt(u.ParameterAt(i/4.),v.ParameterAt(j/4.)))for i in range(5)for j in range(5)]))
                if op.get('inspection',False):faces[-1]['definition']=host['_nurbs_surface_definition'](face)
                if op.get('orientation_inspection',False):
                    faces[-1]['mid_normal']=host['_xyz'](face.NormalAt(u.Mid,v.Mid))
                    faces[-1]['surface_type']=face.UnderlyingSurface().GetType().Name
                if op.get('orientation_witness',False):
                    success,cu,cv=face.ClosestPoint(box.Center)
                    closest=dict(success=bool(success),center=host['_xyz'](box.Center))
                    if success:closest.update(uv=[float(cu),float(cv)],point=host['_xyz'](face.PointAt(cu,cv)),normal=host['_xyz'](face.NormalAt(cu,cv)))
                    faces[-1]['closest_center']=closest
            faces.sort(key=lambda f:(f['domain_v'],f['domain_u']))
            box=b.GetBoundingBox(True)
            records.append(dict(source=None,selected=bool(obj.IsSelected(False)),is_solid=bool(b.IsSolid),
                face_count=int(b.Faces.Count),edge_count=int(b.Edges.Count),vertex_count=int(b.Vertices.Count),
                bounds=[host['_xyz'](box.Min),host['_xyz'](box.Max)],faces=faces))
            if op.get('orientation_inspection',False):records[-1]['solid_orientation']=int(b.SolidOrientation)
        return records
    try:
        doc.ModelAbsoluteTolerance=tolerance['absolute'];doc.ModelRelativeTolerance=tolerance['relative'];doc.ModelAngleToleranceRadians=tolerance['angular']
        R.RhinoApp.RunScript('_-CreaseSplitting _Enable',False)
        success,_,crease_events=observe_command(R.Commands.Command,'CreaseSplitting',
            lambda:R.RhinoApp.RunScript('_-CreaseSplitting _'+('Enable'if op['crease_splitting']else'Disable'),True),lambda:None,lambda:[],True)
        if not success:raise ValueError('could not establish crease policy')
        for definition in op['curves']:owned.append(host['_nurbs_curve_from_definition'](definition))
        record=doc.BeginUndoRecord('Owned Loft splitting profiles')
        try:
            for curve in owned:
                key=doc.Objects.AddCurve(curve)
                if key==S.Guid.Empty:raise ValueError('could not insert Loft source')
                ids.append(key);crcs.append(int(doc.Objects.FindId(key).Geometry.DataCRC(0)))
            if op['preselect']:
                for key in ids:doc.Objects.Select(key)
        finally:doc.EndUndoRecord(record)
        doc.ClearUndoRecords(True)
        marker='Owned loft splitting '+str(S.Guid.NewGuid());R.RhinoApp.WriteLine(marker)
        select=''if op['preselect']else' '+ ' '.join('_SelID '+str(key)for key in ids)+' _Enter'
        seam=(' _Natural _Enter'if op.get('natural',False)else' _Enter')if all(curve.IsClosed for curve in owned)else''
        macro='_-Loft'+select+seam+' _Type=_'+op['style']+' _Closed=_'+('Yes'if op['closed']else'No')
        if op.get('explicit_options',True):macro+=' _Simplify=_None _SplitAtTangents=_'+('Yes'if op['split_at_tangents']else'No')
        macro+=' _Enter _Cancel _Cancel _Cancel'
        success,after,events=observe_command(R.Commands.Command,'Loft',lambda:R.RhinoApp.RunScript(macro,True),snapshot,lambda:[],True)
        history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip()
        if not success or 'Unknown command:'in history or not any(r['source']is None for r in after):
            raise ValueError('Loft splitting command failed: '+history)
        R.RhinoApp.RunScript('_Undo',False);undo=snapshot()
        R.RhinoApp.RunScript('_Redo',False);redo=snapshot()
        return dict(after=after,undo=undo,redo=redo,events=events,history=history,crease_events=crease_events),0
    finally:
        errors=[]
        def clean(action):
            try:action()
            except Exception as error:errors.append(str(error))
        clean(lambda:R.RhinoApp.RunScript('!',False))
        objects=[];clean(lambda:objects.extend(list(doc.Objects)))
        for obj in objects:clean(lambda obj=obj:doc.Objects.Delete(obj.Id,True))
        for curve in reversed(owned):clean(curve.Dispose)
        for name,value in zip(['ModelAbsoluteTolerance','ModelRelativeTolerance','ModelAngleToleranceRadians'],saved):
            clean(lambda name=name,value=value:setattr(doc,name,value))
        clean(lambda:R.RhinoApp.RunScript('_-CreaseSplitting _Enable',False))
        if errors:raise ValueError('Loft splitting cleanup failed: '+str(errors))
