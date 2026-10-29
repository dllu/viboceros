"""Actual MergeFaces on fixed public 3DM seeds with custom face data."""
import json
import os
try:
    from .subd_face_custom_cases import CASES,validate
except (ImportError,ValueError):
    from subd_face_custom_cases import CASES,validate


def run(op,host,iterations,tolerance):
    validate(op)
    from subd_edit_scene import Scene
    from subd_state import snapshot
    from merge_faces_command_input import run as drive
    if type(iterations)is not int or iterations!=1 or host['Rhino'].Commands.Command.InCommand():
        raise ValueError('owned idle custom-face workflow required')
    return workflow(op,CASES[op['case']],host,Scene,'subd_face_custom_assets.json')


def workflow(op,case,host,scene_type,asset_file):
    from subd_state import snapshot
    from merge_faces_command_input import run as drive
    with open(os.path.join(os.path.dirname(__file__),'fixtures',asset_file))as stream:
        assets=json.load(stream)
    with scene_type(dict(sources=[dict(native_archive=assets[op['case']])]),host)as scene:
        def state():
            rows=snapshot(scene)
            for row,obj in zip(rows,sorted(list(scene.doc.Objects),key=lambda o:o.RuntimeSerialNumber)):
                if 'subd'in row:row['texture_coordinate_type']=int(obj.Geometry.TextureCoordinateType)
            return rows
        option=drive(host,'Selected');before=state();archive=scene.archive();steps=[]
        for requested in case['steps']:
            obj=scene.doc.Objects.FindId(scene.ids[0]);obj.UnselectAllSubObjects();g=obj.Geometry
            vertices=sorted(scene.vertices(g),key=lambda v:v.Id);faces=sorted(list(g.Faces),key=lambda f:f.Id)
            index={int(v.Id):i for i,v in enumerate(vertices)};selected=[]
            for pick in requested:
                if pick['kind']=='face':c=faces[pick['index']]
                elif pick['kind']=='vertex':c=vertices[pick['index']]
                else:
                    edges=[e for e in g.Edges if sorted([index[int(e.VertexFrom.Id)],index[int(e.VertexTo.Id)]])==sorted(pick['vertices'])]
                    if len(edges)!=1:raise ValueError('ambiguous owned edge')
                    c=edges[0]
                if not obj.SelectSubObject(c.ComponentIndex(),True,True,False):raise ValueError('owned component selection failed')
                selected.append(dict(kind=pick['kind'],id=int(c.Id)))
            events=[]
            def ended(sender,event):
                if event.CommandEnglishName=='MergeFaces':events.append(str(event.CommandResult))
            scene.R.Commands.Command.EndCommand+=ended
            try:interaction=drive(host)
            finally:scene.R.Commands.Command.EndCommand-=ended
            if events!=['Success']:raise ValueError('MergeFaces did not finish successfully')
            steps.append(dict(selected=selected,interaction=interaction,command_result=events[0],after=state(),after_3dm=scene.archive()))
        undone=[];redone=[]
        for unused in steps:
            scene.R.RhinoApp.RunScript('_Undo',False);undone.append(dict(state=state(),archive=scene.archive()))
        for unused in steps:
            scene.R.RhinoApp.RunScript('_Redo',False);redone.append(dict(state=state(),archive=scene.archive()))
        return dict(before=before,source_3dm=archive,steps=steps,undone=undone,redone=redone,option_input=option),0
