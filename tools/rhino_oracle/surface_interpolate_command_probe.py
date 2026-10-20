"""Fixed public InterpCrvOnSrf getters in an empty owned document."""
import re,math
from copy import deepcopy
OPEN=[[-4.,-3.,0.],[-2.,2.,0.],[0.,-1.5,0.],[2.,3.,0.],[4.,-2.,0.]]
CLOSED=[[-3.,-3.,0.],[3.,-2.5,0.],[2.,3.,0.],[-2.,2.,0.]]
CASES={
    'plane_two':('plane',[[-4.,-3.,0.],[4.,3.,0.],'Enter']),
    'plane_five':('plane',OPEN+['Enter']),
    'plane_close':('plane',CLOSED+['Close']),
    'undo':('plane',OPEN[:3]+['Undo']+OPEN[3:]+['Enter']),
    'cancel':('plane',OPEN[:2]+['Cancel']),
    'hole':('hole',[[-4.,0.,0.],[0.,0.,0.],[4.,1.,0.],'Enter']),
    'cylinder_axis':('cylinder',[[3.,0.,1.],[3.,0.,3.],[3.,0.,5.],'Enter']),
    'cylinder_seam':('cylinder',[[3.*math.cos(t),3.*math.sin(t),z]for t,z in[(-.3,1.),(-.1,3.),(.2,4.5)]]+['Enter']),
    'general':('general',[[.4,.5,.05],[1.,1.3,.429],[1.6,1.5,.84],'Enter']),
    'autoclose':('plane',CLOSED[:3]+[dict(click=CLOSED[0]),'Enter']),
    'persistent':('plane',['PersistentClose=Yes']+CLOSED+['Enter']),
    'autoclose_no':('plane',['PersistentClose=No','AutoClose=No']+CLOSED[:3]+CLOSED[:1]+['Enter']),
}
def request():
    return deepcopy(dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=[dict(op='surface_interpolate_command',id=k,case=k,actions=v[1])for k,v in CASES.items()]))
def validate(op):
    if not isinstance(op,dict)or set(op)!={'op','id','case','actions'}or op['op']!='surface_interpolate_command'or not isinstance(op['case'],str)or op['case']not in CASES:raise ValueError('fixed surface interpolation command required')
    if not isinstance(op['id'],str)or re.match(r'\A[A-Za-z0-9_-]{1,80}\Z',op['id'])is None or op['actions']!=CASES[op['case']][1]:raise ValueError('fixed command actions required')
def validate_request(q):
    if type(q.get('iterations'))is not int or q['iterations']!=1 or not isinstance(q.get('operations'),list)or not 1<=len(q['operations'])<=16:raise ValueError('bounded one-iteration interpolation command request required')
    for op in q['operations']:validate(op)
    if len({op['id']for op in q['operations']})!=len(q['operations']):raise ValueError('unique ids required')
def run(op,host,iterations,tolerance):
    validate(op)
    from surface_short_path_probe import source
    from short_path_command_probe import boundary_source
    from join_probe import observe_command
    from conic_getter_input import drive
    R=host['Rhino'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc
    if iterations!=1 or list(doc.Objects)or doc.UndoRecordingIsActive:raise ValueError('empty owned idle document required')
    case=CASES[op['case']][0]
    surface,boundaries=boundary_source('hole_horizontal',host)if case=='hole'else(source(case,host),None)
    old=doc.Layers.CurrentLayerIndex;layers=[];identifier=None
    def snapshot():
        rows=[]
        for o in sorted(list(doc.Objects),key=lambda o:o.RuntimeSerialNumber):
            row=dict(role='target'if o.Id==identifier else'output',selected=bool(o.IsSelected(False)),layer=doc.Layers[o.Attributes.LayerIndex].Name,name=o.Attributes.Name,crc=int(o.Geometry.DataCRC(0)))
            if isinstance(o.Geometry,G.Curve):row['curve']=host['_nurbs_curve_definition'](o.Geometry);row['closed']=bool(o.Geometry.IsClosed)
            rows.append(row)
        return rows
    try:
        record=doc.BeginUndoRecord('Owned surface interpolation source')
        try:
            for name in ['Surface interpolation source','Surface interpolation output']:
                layer=R.DocObjects.Layer();layer.Name=name;layers.append(doc.Layers.Add(layer))
            a=R.DocObjects.ObjectAttributes();a.LayerIndex=layers[0];a.Name='Target'
            identifier=doc.Objects.AddBrep(surface,a)if isinstance(surface,G.Brep)else doc.Objects.AddSurface(surface,a)
            doc.Layers.SetCurrentLayerIndex(layers[1],True);doc.Objects.Select(identifier)
        finally:doc.EndUndoRecord(record)
        doc.ClearUndoRecords(True);before=snapshot();traces=[]
        marker='Owned InterpCrvOnSrf '+str(host['System'].Guid.NewGuid());R.RhinoApp.WriteLine(marker)
        def execute():
            success,trace=drive(op,host,[identifier],'InterpCrvOnSrf');traces.append(trace);return success
        success,after,events=observe_command(R.Commands.Command,'InterpCrvOnSrf',execute,snapshot,lambda:[],True)
        after_script=snapshot();history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip()
        R.RhinoApp.RunScript('_Undo',False);undo=snapshot();R.RhinoApp.RunScript('_Redo',False);redo=snapshot()
        basis=surface.Faces[0].UnderlyingSurface()if isinstance(surface,G.Brep)else surface
        return dict(target_definition=host['_interchange_brep_record'](surface,include_samples=False)if isinstance(surface,G.Brep)else None,surface=host['_nurbs_surface_definition'](basis),boundaries=boundaries,success=success,before=before,after=after,after_script=after_script,undo=undo,redo=redo,input_trace=traces[0],events=events,history=history),0
    finally:
        doc.Layers.SetCurrentLayerIndex(old,True)
        for o in list(doc.Objects):doc.Objects.Delete(o.Id,True)
        for index in reversed(layers):doc.Layers.Delete(index,True)
        surface.Dispose()
