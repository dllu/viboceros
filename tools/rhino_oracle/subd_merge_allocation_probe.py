"""Actual allocating commands and public SDK/archive copies after MergeFaces."""
try:
    from .subd_merge_allocation_cases import CASES,validate
except (ImportError,ValueError):
    from subd_merge_allocation_cases import CASES,validate


def run(op,host,iterations,tolerance):
    validate(op)
    from subd_edit_scene import Scene
    from subd_state import snapshot,geometry_value
    from merge_faces_command_input import run as drive
    if iterations!=1 or host['Rhino'].Commands.Command.InCommand():raise ValueError('owned idle allocation workflow required')
    case=CASES[op['case']]
    with Scene(case,host)as scene:
        R,S=scene.R,scene.S
        option=drive(host,'Selected');before=snapshot(scene);source_archive=scene.archive()
        obj=scene.doc.Objects.FindId(scene.ids[0]);faces=sorted(list(obj.Geometry.Faces),key=lambda f:f.Id)
        for i in case['faces']:
            if not obj.SelectSubObject(faces[i].ComponentIndex(),True,True,False):raise ValueError('owned face preselection failed')
        events=[];completed=[]
        def ended(sender,event):
            if event.CommandEnglishName in('MergeFaces','SubDivide'):events.append(dict(command=event.CommandEnglishName,result=str(event.CommandResult)))
            if event.CommandEnglishName=='SubDivide':completed.append(snapshot(scene))
        R.Commands.Command.EndCommand+=ended
        try:
            interaction=drive(host)
            merged=snapshot(scene);merged_archive=scene.archive()
            obj=scene.doc.Objects.FindId(scene.ids[0])
            def sdk(g):
                try:
                    initial=geometry_value(g,host)
                    if not g.Subdivide(1):raise ValueError('public SubD subdivision failed')
                    return dict(before=initial,after=geometry_value(g,host))
                finally:g.Dispose()
            copied=sdk(obj.Geometry.Duplicate())
            model=R.FileIO.File3dm.FromByteArray(S.Convert.FromBase64String(merged_archive))
            try:
                candidates=[o.Geometry for o in model.Objects if isinstance(o.Geometry,R.Geometry.SubD)and o.Attributes.Name=='Input 0']
                if len(candidates)!=1:raise ValueError('owned archived SubD missing')
                archived=sdk(candidates[0].Duplicate())
            finally:model.Dispose()
            scene.doc.Objects.UnselectAll();obj.UnselectAllSubObjects()
            local_face=None
            if case['local']:
                face=min(list(obj.Geometry.Faces),key=lambda f:f.Id);local_face=int(face.Id)
                if not obj.SelectSubObject(face.ComponentIndex(),True,True,False):raise ValueError('local face preselection failed')
            elif not obj.Select(True):raise ValueError('whole SubD preselection failed')
            started=S.DateTime.UtcNow
            R.RhinoApp.SendKeystrokes('_SubDivide',True)
            while len(completed)==0 and (S.DateTime.UtcNow-started).TotalSeconds<15:
                R.RhinoApp.Wait()
            if len(completed)==0:
                R.RhinoApp.SendKeystrokes('_Cancel',True);R.RhinoApp.Wait()
                raise ValueError('queued SubDivide did not complete')
            R.RhinoApp.Wait();returned=True
        finally:R.Commands.Command.EndCommand-=ended
        if events!=[dict(command='MergeFaces',result='Success'),dict(command='SubDivide',result='Success')]or len(completed)!=1 or R.Commands.Command.InCommand():raise ValueError('allocating command sequence failed: '+str(events))
        after=snapshot(scene);after_archive=scene.archive();undone=[];redone=[]
        for unused in range(2):R.RhinoApp.RunScript('_Undo',False);undone.append(snapshot(scene))
        for unused in range(2):R.RhinoApp.RunScript('_Redo',False);redone.append(snapshot(scene))
        return dict(before=before,source_3dm=source_archive,merged=merged,merged_3dm=merged_archive,
                    sdk_copy=copied,sdk_archive=archived,local_face=local_face,after=after,after_endcommand=completed[0],after_3dm=after_archive,
                    undone=undone,redone=redone,events=events,interaction=interaction,option_input=option,returned=bool(returned),transport='RhinoApp.SendKeystrokes'),0
