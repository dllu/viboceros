"""Public HidePt/ShowPt command and grip visibility observations."""
import re

COMMANDS = ('Move','Undo','Redo','HidePt','ShowPt','SelPt','InvertPt','SelNone','NextU','PrevU','AddNextU','AddPrevU','SelU','SelV','PointsOn','EditPtOn','PointsOff','NextV','PrevV','AddNextV','AddPrevV','SelUV')
KINDS = ('curve','rational','closed','periodic','line','polyline','circle','arc','surface','surface_closed','mesh','polycurve','surface_periodic_u','surface_periodic_v')


def validate(op):
    if (not isinstance(op,dict) or set(op) != {'op','id','artifact_path','sources','actions'} or op.get('op') != 'grip_visibility'
            or not isinstance(op.get('id'),str) or re.match(r'^[A-Za-z0-9_-]{1,80}\Z',op['id']) is None
            or not isinstance(op.get('artifact_path'),str) or not isinstance(op.get('sources'),list) or not 1 <= len(op['sources']) <= 4
            or not isinstance(op.get('actions'),list) or not 1 <= len(op['actions']) <= 64): raise ValueError('invalid grip visibility recipe')
    for s in op['sources']:
        if (not isinstance(s,dict) or set(s) != {'kind','mode','parent'} or s['kind'] not in KINDS or s['mode'] not in ('control','edit')
                or type(s['parent']) is not bool or s['mode']=='edit' and (s['kind'].startswith('surface') or s['kind']=='mesh')): raise ValueError('invalid visibility source')
    for a in op['actions']:
        if (not isinstance(a,dict) or set(a)-{'command','seed','parents','pick','cancel'} or a.get('command') not in COMMANDS
                or 'seed'in a and a['seed'] not in ('first','last','middle','before_middle','ends','all','empty')
                or 'parents'in a and type(a['parents']) is not bool
                or 'pick'in a and (a['command']!='HidePt' or a['pick']!='all')
                or 'cancel'in a and (a['command']!='HidePt' or type(a['cancel'])is not bool)):raise ValueError('invalid visibility action')


def run(op,host):
    validate(op)
    from grip_navigation_probe import create_source,seed_indices
    from join_probe import observe_command
    R,S=host['Rhino'],host['System'];doc=R.RhinoDoc.ActiveDoc
    if list(doc.Objects) or R.Commands.Command.InCommand():raise ValueError('visibility capture needs an empty owned idle document')
    ids=[];originals=[];mutating=[False]
    edited=any(a['command']in ('Move','Undo','Redo')for a in op['actions'])
    if edited and any(s['kind']!='curve'for s in op['sources']):raise ValueError('edited visibility recipes require cubic curves')
    def snapshot():
        rows=[]
        for key,original in zip(ids,originals):
            obj=doc.Objects.FindId(key)
            if obj is None or not mutating[0]and not R.Geometry.GeometryBase.GeometryEquals(original,obj.Geometry):raise ValueError('visibility changed source identity or geometry')
            rows.append(dict(selected=bool(obj.IsSelected(False)),visible=not bool(obj.IsHidden),grips_on=bool(obj.GripsOn),
                grips=[dict(index=int(g.Index),point=host['_xyz'](g.CurrentLocation),selected=bool(g.IsSelected(False)),hidden=bool(g.IsHidden),selectable=bool(g.IsSelectable(True,False,False,False)))for g in (obj.GetGrips()or[])]))
        if edited:
            for key,row in zip(ids,rows):row['curve']=host['_nurbs_curve_definition'](doc.Objects.FindId(key).Geometry)
        return dict(sources=rows,peer_selected=bool(doc.Objects.FindId(peer).IsSelected(False)))
    try:
        record=doc.BeginUndoRecord('Grip visibility sources')
        try:
            for s in op['sources']:
                geometry=create_source(s['kind'],host)
                try:key=doc.Objects.Add(geometry)
                finally:geometry.Dispose()
                if key==S.Guid.Empty:raise ValueError('cannot add grip visibility source')
                ids.append(key);originals.append(doc.Objects.FindId(key).Geometry.Duplicate())
            peer=doc.Objects.AddPoint(host['_point']([20.,20.,20.]))
        finally:doc.EndUndoRecord(record)
        model=R.FileIO.File3dm()
        path=op['artifact_path']
        if path.startswith('/'):path='Z:'+path.replace('/','\\')
        try:
            for key in ids:
                obj=doc.Objects.FindId(key);model.Objects.Add(obj.Geometry,obj.Attributes)
            if not model.Write(path,8):raise ValueError('cannot save visibility sources')
        finally:model.Dispose()
        for key,s in zip(ids,op['sources']):
            if s['mode']=='edit':
                doc.Objects.Select(key,True);R.RhinoApp.RunScript('_EditPtOn',False)
            else:doc.Objects.FindId(key).GripsOn=True
        record=doc.BeginUndoRecord('Grip visibility Redo witness')
        try:marker=doc.Objects.AddPoint(host['_point']([30.,30.,30.]))
        finally:doc.EndUndoRecord(record)
        R.RhinoApp.RunScript('_Undo',False)
        if doc.Objects.FindId(marker)is not None:raise ValueError('cannot prepare visibility Redo witness')
        for key,s in zip(ids,op['sources']):
            if s['parent']:doc.Objects.Select(key,True)
        doc.Objects.Select(peer,True)
        before=snapshot();steps=[]
        for action in op['actions']:
            if 'parents'in action:
                for key in ids:doc.Objects.Select(key,action['parents'])
            if 'seed'in action:
                for key in ids:
                    grips=doc.Objects.FindId(key).GetGrips()or[];indices=([max(0,len(grips)//2-1)]if action['seed']=='before_middle'else seed_indices(action['seed'],len(grips)))if grips else[]
                    for g in grips:g.Select(int(g.Index)in indices)
            initial=snapshot();name=action['command'];macro='_'+name
            if name=='Move':
                visible=any(g.IsSelected(False)and not g.IsHidden for key in ids for g in (doc.Objects.FindId(key).GetGrips()or[]))
                macro='_Move w0,0,0 w0,0,1'if visible else'_Move'
            if name=='SelUV':
                if len(ids)!=1 or not op['sources'][0]['kind'].startswith('surface'):raise ValueError('SelUV visibility witness requires one surface')
                grips=doc.Objects.FindId(ids[0]).GetGrips();point=host['_xyz'](grips[len(grips)//2].CurrentLocation)
                macro='_SelUV _SelID '+str(ids[0])+' _Direction=_Both _HideOthers=_No w'+host['_command_point'](point)+' _Enter'
            host['_record_progress'](op['id']+' '+macro)
            inputs=[]
            def execute():
                if name not in ('HidePt','Move'):return R.RhinoApp.RunScript(macro,False)
                from hide_pt_input import run as hide
                returned,getter,text=hide(host,action.get('pick')=='all',action.get('cancel',False),command=name,has_points=visible if name=='Move'else True)
                inputs.append(dict(opened=getter,input=text))
                return returned
            mutating[0]=name in ('Move','Undo','Redo')
            _,after,events=observe_command(R.Commands.Command,name,execute,snapshot,lambda:[],True)
            results=[e['result']for e in events if e['name']==name]
            if len(results)!=1:raise ValueError('missing visibility command completion')
            scripted=snapshot()
            if mutating[0]:
                for i,key in enumerate(ids):
                    originals[i].Dispose();originals[i]=doc.Objects.FindId(key).Geometry.Duplicate()
            mutating[0]=False
            steps.append(dict(command=name,macro=macro,before=initial,after=after,after_script=scripted,result=results[0],getter=inputs[0]if inputs else None))
        R.RhinoApp.RunScript('_Redo',False)
        value=dict(before=before,steps=steps,redo_retained=doc.Objects.FindId(marker)is not None)
        import json
        with open(path+'.capture.json','w')as stream:json.dump(value,stream,indent=2,allow_nan=False)
        return value,0
    finally:
        for g in originals:g.Dispose()
        for obj in list(doc.Objects):
            if obj.GripsOn:obj.GripsOn=False
            doc.Objects.Delete(obj.Id,True)
