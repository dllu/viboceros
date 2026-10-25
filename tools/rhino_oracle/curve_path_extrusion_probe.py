"""Owned ExtrudeCrvAlongCrv document edits with explicit global crease policy."""
import math


def validate(op):
    if (not isinstance(op,dict) or set(op) != {'op','id','profile','path','crease_splitting','split_at_tangents','solid','delete_input','preselect'}
            or op['op'] != 'curve_path_extrusion' or any(type(op[k]) is not bool for k in ['crease_splitting','split_at_tangents','solid','delete_input','preselect'])):
        raise ValueError('invalid curve path extrusion options')
    for key in ['profile','path']:
        curve=op[key]
        if (not isinstance(curve,dict) or set(curve) != {'degree','knots','control_points'}
                or type(curve['degree']) is not int or not 1 <= curve['degree'] <= 64
                or not isinstance(curve['control_points'],list) or not curve['degree'] < len(curve['control_points']) <= 10000
                or not isinstance(curve['knots'],list) or len(curve['knots']) != len(curve['control_points'])+curve['degree']+1):
            raise ValueError('invalid curve path extrusion curve dimensions')
        for cp in curve['control_points']:
            if (not isinstance(cp,dict) or set(cp) != {'point','weight'} or not isinstance(cp['point'],list) or len(cp['point']) != 3):
                raise ValueError('invalid curve path extrusion control')
            values=cp['point']+[cp['weight']]
            if any(type(x) not in (int,float) or math.isnan(x) or math.isinf(x) for x in values) or cp['weight'] == 0:
                raise ValueError('invalid curve path extrusion coordinates or weight')
        if (any(type(x) not in (int,float) or math.isnan(x) or math.isinf(x) for x in curve['knots'])
                or any(a>b for a,b in zip(curve['knots'],curve['knots'][1:]))):
            raise ValueError('invalid curve path extrusion knots')


def run(op,host,iterations,tolerance):
    validate(op)
    try:
        from .join_probe import observe_command
    except (ImportError,ValueError,SystemError):
        from join_probe import observe_command
    R,S=host['Rhino'],host['System'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc
    if iterations!=1 or list(doc.Objects) or doc.UndoRecordingIsActive:
        raise ValueError('empty owned idle document required')
    saved=(doc.ModelAbsoluteTolerance,doc.ModelRelativeTolerance,doc.ModelAngleToleranceRadians)
    owned=[];ids=[];crcs=[]
    def snapshot():
        result=[]
        for obj in sorted(list(doc.Objects),key=lambda o:o.RuntimeSerialNumber):
            if obj.Id in ids:
                result.append(dict(source=ids.index(obj.Id),unchanged=int(obj.Geometry.DataCRC(0))==crcs[ids.index(obj.Id)],selected=bool(obj.IsSelected(False))))
                continue
            b=obj.Geometry
            if not isinstance(b,G.Brep) or not b.IsValid:
                raise ValueError('curve path extrusion output is not a valid B-rep')
            mass=None
            try:
                volume=None
                if b.IsSolid:
                    mass=G.VolumeMassProperties.Compute(b)
                    if mass is None:raise ValueError('curve path volume failed')
                    volume=abs(float(mass.Volume))
                box=b.GetBoundingBox(True)
                result.append(dict(source=None,selected=bool(obj.IsSelected(False)),is_solid=bool(b.IsSolid),
                    face_count=int(b.Faces.Count),volume=volume,bounds=[host['_xyz'](box.Min),host['_xyz'](box.Max)]))
            finally:
                if mass is not None:mass.Dispose()
        return result
    try:
        doc.ModelAbsoluteTolerance=tolerance['absolute'];doc.ModelRelativeTolerance=tolerance['relative'];doc.ModelAngleToleranceRadians=tolerance['angular']
        # This Rhino build's public SDK has no getter for this switch. This owned private
        # capture establishes/restores an enabled baseline and observes the
        # public command's completion; the paired output geometry verifies its effect.
        R.RhinoApp.RunScript('_-CreaseSplitting _Enable',False)
        crease_success,_,crease_events=observe_command(R.Commands.Command,'CreaseSplitting',
            lambda:R.RhinoApp.RunScript('_-CreaseSplitting _'+('Enable' if op['crease_splitting'] else 'Disable'),True),
            lambda:None,lambda:[],True)
        if not crease_success:raise ValueError('could not set crease splitting')
        for key in ['profile','path']:
            owned.append(host['_nurbs_curve_from_definition'](op[key]))
        record=doc.BeginUndoRecord('Owned curve path extrusion sources')
        try:
            for curve in owned:
                key=doc.Objects.AddCurve(curve)
                if key==S.Guid.Empty:raise ValueError('curve path source insertion failed')
                ids.append(key);crcs.append(int(doc.Objects.FindId(key).Geometry.DataCRC(0)))
            if op['preselect']:doc.Objects.Select(ids[0])
        finally:doc.EndUndoRecord(record)
        doc.ClearUndoRecords(True)
        marker='Owned curve path '+str(S.Guid.NewGuid());R.RhinoApp.WriteLine(marker)
        select='' if op['preselect'] else ' _SelID '+str(ids[0])+' _Enter'
        macro='_-ExtrudeCrvAlongCrv'+select+' _Output=_Surface _Solid=_'+('Yes' if op['solid'] else 'No')
        macro+=' _DeleteInput=_'+('Yes' if op['delete_input'] else 'No')+' _SplitAtTangents=_'+('Yes' if op['split_at_tangents'] else 'No')
        macro+=" '_-SelID "+str(ids[1])+' _Cancel _Cancel _Cancel'
        success,after,events=observe_command(R.Commands.Command,'ExtrudeCrvAlongCrv',lambda:R.RhinoApp.RunScript(macro,True),snapshot,lambda:[],True)
        history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip()
        if 'Unknown command:' in history or (success and not any(r['source'] is None for r in after)):
            raise ValueError('curve path extrusion command failed: '+history)
        undo=redo=None
        if success:
            R.RhinoApp.RunScript('_Undo',False);undo=snapshot()
            R.RhinoApp.RunScript('_Redo',False);redo=snapshot()
        elif any(r['source'] is None for r in after):
            raise ValueError('failed curve path command left output objects: '+history)
        return dict(succeeded=success,after=after,undo=undo,redo=redo,history=history,events=events,
                    crease_splitting=op['crease_splitting'],crease_events=crease_events),0
    finally:
        errors=[]
        def cleanup(action):
            try:action()
            except Exception as error:errors.append(str(error))
        cleanup(lambda:R.RhinoApp.RunScript('!',False))
        objects=[];cleanup(lambda:objects.extend(list(doc.Objects)))
        for obj in objects:cleanup(lambda obj=obj:doc.Objects.Delete(obj.Id,True))
        for curve in reversed(owned):cleanup(curve.Dispose)
        for name,value in zip(['ModelAbsoluteTolerance','ModelRelativeTolerance','ModelAngleToleranceRadians'],saved[:3]):
            cleanup(lambda name=name,value=value:setattr(doc,name,value))
        cleanup(lambda:R.RhinoApp.RunScript('_-CreaseSplitting _Enable',False))
        if errors:raise ValueError('curve path extrusion cleanup failed: '+str(errors))
