"""Public mouse callbacks, native 3DM outputs and SubD insertion histories."""
import json
import os
try:
    from .subd_insert_custom_cases import CASES,validate
except (ImportError,ValueError):
    from subd_insert_custom_cases import CASES,validate


def run(op,host,iterations,tolerance):
    validate(op)
    return workflow(op,CASES[op['case']],host,iterations,tolerance)


def workflow(op,case,host,iterations,tolerance):
    from subd_edit_scene import Scene
    from subd_state import snapshot
    from conic_getter_input import drive
    from merge_faces_command_input import run as merge
    if type(iterations)is not int or iterations!=1 or host['Rhino'].Commands.Command.InCommand():
        raise ValueError('owned idle SubD insertion workflow required')
    asset='subd_face_custom_assets.json'if case['asset']=='base'else'subd_face_custom_holdout_assets.json'
    with open(os.path.join(os.path.dirname(__file__),'fixtures',asset))as stream:seed=json.load(stream)[case['seed']]
    with Scene(dict(sources=[dict(native_archive=seed)]),host)as scene:
        initial=snapshot(scene);initial_archive=scene.archive();merge_result=None
        if case['merge']:
            merge(host,'Selected');obj=scene.doc.Objects.FindId(scene.ids[0])
            faces=sorted(list(obj.Geometry.Faces),key=lambda f:f.Id)
            for index in case['merge']:
                if not obj.SelectSubObject(faces[index].ComponentIndex(),True,True,False):raise ValueError('owned merge preselection failed')
            events=[]
            def merged(sender,event):
                if event.CommandEnglishName=='MergeFaces':events.append(str(event.CommandResult))
            scene.R.Commands.Command.EndCommand+=merged
            try:merge(host)
            finally:scene.R.Commands.Command.EndCommand-=merged
            if events!=['Success']:raise ValueError('owned merge failed')
            merge_result=dict(state=snapshot(scene),archive=scene.archive())
        obj=scene.doc.Objects.FindId(scene.ids[0]);obj.UnselectAllSubObjects();scene.doc.Objects.UnselectAll()
        if not obj.Select(True):raise ValueError('owned SubD selection failed')
        before=snapshot(scene);source=scene.archive();view=scene.doc.Views.ActiveView
        view.ActiveViewport.SetProjection(scene.R.Display.DefinedViewportProjection.Top,'Owned SubD insertion',False)
        view.ActiveViewport.ZoomBoundingBox(obj.Geometry.GetBoundingBox(True));view.Redraw()
        events=[]
        def ended(sender,event):
            if event.CommandEnglishName=='InsertPoint':events.append(str(event.CommandResult))
        operation=dict(op='insert_point_command',id=op['id'],actions=case['actions'],capture_pick_hint=True)
        scene.R.Commands.Command.EndCommand+=ended
        try:returned,trace=drive(operation,host,[],command='InsertPoint')
        finally:scene.R.Commands.Command.EndCommand-=ended
        if len(events)!=1 or scene.R.Commands.Command.InCommand():raise ValueError('owned insertion did not finish once')
        after=snapshot(scene);after_archive=scene.archive()
        undone=[];redone=[]
        for unused in range(2 if case['merge']and events==['Success']else 1):
            scene.R.RhinoApp.RunScript('_Undo',False);undone.append(dict(state=snapshot(scene),archive=scene.archive()))
        for unused in undone:
            scene.R.RhinoApp.RunScript('_Redo',False);redone.append(dict(state=snapshot(scene),archive=scene.archive()))
        return dict(initial=initial,initial_3dm=initial_archive,merge=merge_result,before=before,source_3dm=source,
                    returned=bool(returned),events=events,trace=trace,after=after,after_3dm=after_archive,
                    undone=undone,redone=redone),0
