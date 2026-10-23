"""Owned edge-spin scenes share the exact source and history recorder."""
try:
    from .subd_spin_edge_cases import ALL_CASES,validate
    from .crease_probe import run_scene
except (ImportError,ValueError):
    from subd_spin_edge_cases import ALL_CASES,validate
    from crease_probe import run_scene
def run(operation,host,iterations,tolerance):
    validate(operation)
    case=ALL_CASES[operation['case']]
    if not case.get('live'):return run_scene(case,host,iterations,tolerance)
    if iterations!=1:raise ValueError('one edge spin workflow required')
    from subd_edit_scene import Scene
    from conic_getter_input import drive
    from join_probe import observe_command
    with Scene(case,host)as scene:
        if case.get('grips'):scene.select()
        before=scene.snapshot();archive=scene.archive()
        view=scene.doc.Views.ActiveView
        view.ActiveViewport.SetProjection(scene.R.Display.DefinedViewportProjection.Top,'Owned edge spin',False)
        view.ActiveViewport.ZoomBoundingBox(scene.R.Geometry.BoundingBox([v.ControlNetPoint for key in scene.ids for v in scene.vertices(scene.doc.Objects.FindId(key).Geometry)]));view.Redraw()
        trace=[];marker='Owned edge spin '+str(scene.S.Guid.NewGuid());scene.R.RhinoApp.WriteLine(marker)
        def execute():
            result,details=drive(dict(op=operation['op'],id=operation['id'],actions=case['actions']),host,[],command='SubDSpinEdge');trace.append(details);return result
        success,after,events=observe_command(scene.R.Commands.Command,'SubDSpinEdge',execute,scene.snapshot,lambda:[],True)
        history=scene.R.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip()
        after_script=scene.snapshot();scene.R.RhinoApp.RunScript('_Undo',False);undo=scene.snapshot();scene.R.RhinoApp.RunScript('_Redo',False);redo=scene.snapshot()
        return dict(input=case,source_3dm=archive,before=before,success=success,after=after,after_script=after_script,undo=undo,redo=redo,events=events,history=history,input_trace=trace),0
