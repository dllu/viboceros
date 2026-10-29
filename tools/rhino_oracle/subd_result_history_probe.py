"""Live result selection changes before Undo and Redo of a completed merge."""
CASES=['keep','clear','other_face','edge','vertex','whole','face_and_edge','face_and_vertex']


def validate(op):
    try:
        ok=(isinstance(op,dict)and set(op)=={'op','id','case'}and op['op']=='subd_result_history'
            and isinstance(op['case'],str)and op['case']in CASES and op['id']==op['case'])
    except TypeError:ok=False
    if not ok:raise ValueError('fixed owned result-selection history case required')


def run(op,host,iterations,tolerance):
    validate(op)
    from subd_edit_scene import Scene
    from subd_state import snapshot
    from subd_merge_allocation_cases import STRIP3
    from merge_faces_command_input import run as drive
    if iterations!=1 or host['Rhino'].Commands.Command.InCommand():raise ValueError('owned idle history workflow required')
    with Scene(dict(sources=[STRIP3]),host)as scene:
        drive(host,'Selected');before=snapshot(scene);archive=scene.archive()
        obj=scene.doc.Objects.FindId(scene.ids[0]);faces=sorted(list(obj.Geometry.Faces),key=lambda f:f.Id)
        for f in faces[:2]:
            if not obj.SelectSubObject(f.ComponentIndex(),True,True,False):raise ValueError('input face preselection failed')
        events=[]
        def ended(sender,event):
            if event.CommandEnglishName=='MergeFaces':events.append(str(event.CommandResult))
        scene.R.Commands.Command.EndCommand+=ended
        try:drive(host)
        finally:scene.R.Commands.Command.EndCommand-=ended
        if events!=['Success']:raise ValueError('merge did not finish')
        after_command=snapshot(scene);obj=scene.doc.Objects.FindId(scene.ids[0]);g=obj.Geometry
        action=op['case']
        if action!='keep':obj.UnselectAllSubObjects()
        selected=[]
        if action in('other_face','face_and_edge','face_and_vertex'):selected.append(max(list(g.Faces),key=lambda f:f.Id))
        if action in('edge','face_and_edge'):selected.append(min(list(g.Edges),key=lambda e:e.Id))
        if action in('vertex','face_and_vertex'):selected.append(min(scene.vertices(g),key=lambda v:v.Id))
        for c in selected:
            if not obj.SelectSubObject(c.ComponentIndex(),True,True,False):raise ValueError('result selection change failed')
        if action=='whole'and not obj.Select(True):raise ValueError('result whole selection failed')
        after_selection=snapshot(scene);after_archive=scene.archive()
        scene.R.RhinoApp.RunScript('_Undo',False);undone=snapshot(scene)
        scene.R.RhinoApp.RunScript('_Redo',False);redone=snapshot(scene)
        obj=scene.doc.Objects.FindId(scene.ids[0]);obj.Select(False);obj.UnselectAllSubObjects()
        result=min(list(obj.Geometry.Faces),key=lambda f:f.Id)
        if not obj.SelectSubObject(result.ComponentIndex(),True,True,False):raise ValueError('original result reselection failed')
        reselected=snapshot(scene)
        scene.R.RhinoApp.RunScript('_Undo',False);reselected_undo=snapshot(scene)
        scene.R.RhinoApp.RunScript('_Redo',False);reselected_redo=snapshot(scene)
        return dict(before=before,source_3dm=archive,after_command=after_command,after_selection=after_selection,
                    after_3dm=after_archive,undone=undone,redone=redone,reselected=reselected,
                    reselected_undo=reselected_undo,reselected_redo=reselected_redo),0
