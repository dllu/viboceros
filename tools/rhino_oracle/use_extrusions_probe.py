"""Public native preference command; never opens or controls a desktop session."""
if __package__:
    from .use_extrusions_cases import validate
else:
    from use_extrusions_cases import validate


def run(op,host,iterations,tolerance):
    validate(op)
    from join_probe import observe_command
    R,S=host['Rhino'],host['System'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc
    if iterations!=1 or list(doc.Objects) or R.Commands.Command.InCommand():raise ValueError('empty owned idle document required')
    original=bool(R.ApplicationSettings.GeneralSettings.UseExtrusions)
    try:
        if not R.RhinoApp.RunScript('_UseExtrusions _'+op['initial'],True):raise ValueError('initial extrusion policy failed')
        key=doc.Objects.AddPoint(G.Point3d(1,2,3));doc.Objects.Select(key)
        doc.ClearUndoRecords(True)
        R.RhinoApp.RunScript('_Point w10,20,30',False);R.RhinoApp.RunScript('_Undo',False)
        def snapshot():
            rows=[dict(point=host['_xyz'](obj.Geometry.Location),selected=int(obj.IsSelected(False)))
                  for obj in sorted(list(doc.Objects),key=lambda o:int(o.RuntimeSerialNumber))]
            return dict(use_extrusions=bool(R.ApplicationSettings.GeneralSettings.UseExtrusions),points=rows)
        before=snapshot();marker='Owned UseExtrusions '+str(S.Guid.NewGuid());R.RhinoApp.WriteLine(marker)
        option=' !' if op['answer']=='Cancel' else ' _'+op['answer']
        macro='_UseExtrusions'+option
        success,after,events=observe_command(R.Commands.Command,'UseExtrusions',lambda:R.RhinoApp.RunScript(macro,True),snapshot,lambda:[],True)
        redone=bool(R.RhinoApp.RunScript('_Redo',True));redo=snapshot()
        return dict(before=before,after=after,redo=redo,success=success,redone=redone,events=events,
                    history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip()),0
    finally:
        R.RhinoApp.RunScript('!',False)
        for obj in list(doc.Objects):doc.Objects.Delete(obj.Id,True)
        doc.ClearUndoRecords(True)
        R.RhinoApp.RunScript('_UseExtrusions _'+('Extrusion' if original else 'Polysurface'),False)
