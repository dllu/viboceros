"""Public per-face colors through MergeFaces and sequential Undo/Redo."""
try:
    from .subd_merge_properties_cases import CASES,validate
except (ImportError,ValueError):
    from subd_merge_properties_cases import CASES,validate


def run(op,host,iterations,tolerance):
    validate(op)
    from subd_edit_scene import Scene
    from merge_faces_command_input import run as drive
    from subd_state import snapshot as state_snapshot
    if iterations!=1 or host['Rhino'].Commands.Command.InCommand():raise ValueError('owned idle workflow required')
    case=CASES[op['case']]
    class ColoredScene(Scene):
        def geometry(self,definition):
            g=Scene.geometry(self,definition)
            for f,color in zip(list(g.Faces),definition['colors']):
                if color is not None:f.PerFaceColor=self.S.Drawing.Color.FromArgb(*color)
            return g
    with ColoredScene(case,host)as scene:
        snapshot=lambda:state_snapshot(scene)
        option=drive(host,'Selected')
        before=snapshot();source_archive=scene.archive();steps=[]
        for requested in case['steps']:
            obj=scene.doc.Objects.FindId(scene.ids[0]);obj.UnselectAllSubObjects()
            g=obj.Geometry;vertices=sorted(scene.vertices(g),key=lambda v:v.Id);faces=sorted(list(g.Faces),key=lambda f:f.Id)
            index={int(v.Id):i for i,v in enumerate(vertices)}
            selected=[]
            for pick in requested:
                if pick['kind']=='face':c=faces[pick['index']]
                elif pick['kind']=='vertex':c=vertices[pick['index']]
                else:
                    matches=[e for e in g.Edges if sorted([index[int(e.VertexFrom.Id)],index[int(e.VertexTo.Id)]])==sorted(pick['vertices'])]
                    if len(matches)!=1:raise ValueError('ambiguous owned edge')
                    c=matches[0]
                if not obj.SelectSubObject(c.ComponentIndex(),True,True,False):raise ValueError('component preselection failed')
                selected.append(dict(kind=pick['kind'],id=int(c.Id)))
            events=[]
            def ended(sender,event):
                if event.CommandEnglishName=='MergeFaces':events.append(str(event.CommandResult))
            scene.R.Commands.Command.EndCommand+=ended
            try:interaction=drive(host)
            finally:scene.R.Commands.Command.EndCommand-=ended
            if len(events)!=1:raise ValueError('MergeFaces did not finish once')
            steps.append(dict(selected=selected,command_result=events[0],interaction=interaction,after=snapshot(),after_3dm=scene.archive()))
        undone=[];redone=[]
        for unused in steps:
            scene.R.RhinoApp.RunScript('_Undo',False);undone.append(snapshot())
        for unused in steps:
            scene.R.RhinoApp.RunScript('_Redo',False);redone.append(snapshot())
        return dict(before=before,source_3dm=source_archive,steps=steps,undone=undone,redone=redone,option_input=option),0
