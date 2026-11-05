"""Observe public type-selection commands without modeling or private APIs."""
if __package__:
    from .type_selection_cases import validate
else:
    from type_selection_cases import validate


def run(op,host,iterations,tolerance):
    validate(op)
    from type_selection_scene import build
    from join_probe import observe_command
    R,S=host['Rhino'],host['System'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc
    if iterations!=1 or list(doc.Objects) or R.Commands.Command.InCommand():
        raise ValueError('empty owned idle document required')
    keys={};layer_index=None
    try:
        keys,layer_index=build(R,S,doc,op)
        original={str(key):int(doc.Objects.FindId(key).Geometry.DataCRC(0)) for key in keys.values()}
        def definition_state():
            return {str(d.Id):[int(o.Geometry.DataCRC(0)) for o in d.GetObjects()]
                    for d in doc.InstanceDefinitions if not d.IsDeleted}
        definitions=definition_state()
        def snapshot():
            rows=[]
            for name,key in sorted(keys.items()):
                obj=doc.Objects.FindId(key);g=obj.Geometry;a=obj.Attributes
                row=dict(name=name,selected=int(obj.IsSelected(False)),groups=int(a.GroupCount),
                         hidden=bool(obj.IsHidden),locked=bool(obj.IsLocked),layer=doc.Layers[a.LayerIndex].Name,
                         user_text=str(a.GetUserString('Owned selection')))
                if isinstance(g,G.InstanceReferenceGeometry):
                    t=g.Xform
                    row.update(kind='instance',definition=doc.InstanceDefinitions.FindId(g.ParentIdefId).Name,
                               transform=[[float(t[i,j]) for j in range(4)] for i in range(3)])
                elif isinstance(g,G.Extrusion):row['kind']='extrusion'
                elif isinstance(g,G.Brep):row['kind']='brep'
                elif isinstance(g,G.Point):row['kind']='point'
                else:raise ValueError('unexpected selection source')
                rows.append(row)
            unchanged=all(int(doc.Objects.FindId(key).Geometry.DataCRC(0))==original[str(key)] for key in keys.values())
            unchanged=unchanged and definitions==definition_state()
            points=[host['_xyz'](obj.Geometry.Location) for obj in doc.Objects if obj.Id not in keys.values() and isinstance(obj.Geometry,G.Point)]
            return dict(sources=rows,geometry_unchanged=unchanged,points=points)
        doc.ClearUndoRecords(True)
        R.RhinoApp.RunScript('_Point w90,91,92',False)
        R.RhinoApp.RunScript('_Undo',False)
        if op['variant']=='preselected' or op.get('preselected'):
            for name in ('point','other'):doc.Objects.Select(keys[name])
        before=snapshot();command=op['command']
        macro='_'+command if command!='SelBlockInstanceNamed' else '_-SelBlockInstanceNamed "'+op['name']+'"'
        marker='Owned type selection '+str(S.Guid.NewGuid());R.RhinoApp.WriteLine(marker)
        success,after,events=observe_command(R.Commands.Command,command,
                lambda:R.RhinoApp.RunScript(macro,True),snapshot,lambda:[],True)
        repeated,repeat,repeat_events=observe_command(R.Commands.Command,command,
                lambda:R.RhinoApp.RunScript(macro,True),snapshot,lambda:[],True)
        R.RhinoApp.RunScript('_SelNone',False);R.RhinoApp.RunScript('_SelPrev',False)
        previous=snapshot()
        redone=bool(R.RhinoApp.RunScript('_Redo',True));redo=snapshot()
        return dict(success=success,repeated=repeated,redone=redone,before=before,after=after,repeat=repeat,
                    previous=previous,redo=redo,events=events+repeat_events,
                    history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip()),0
    finally:
        R.RhinoApp.RunScript('!',False)
        if layer_index is not None:
            layer=doc.Layers[layer_index];layer.IsVisible=True;layer.IsLocked=False;doc.Layers.Modify(layer,layer_index,True)
        for obj in list(doc.Objects):
            doc.Objects.Show(obj.Id,True);doc.Objects.Unlock(obj.Id,True);doc.Objects.Delete(obj.Id,True)
        for d in reversed(list(doc.InstanceDefinitions)):
            if not d.IsDeleted:doc.InstanceDefinitions.Delete(d.Index,True,True)
        for index in range(doc.Groups.Count):doc.Groups.Delete(index)
        doc.ClearUndoRecords(True)
