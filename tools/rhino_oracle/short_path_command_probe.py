"""Fixed public ShortPath getter scenes in an empty owned document."""
import re
from copy import deepcopy
CASES={'plane':[[-4.,-3.,0.],[4.,3.,0.]],'cancel':[[-4.,-3.,0.],'Cancel'],'cylinder':[[3.,0.,1.],[0.,3.,5.]],'general':[[.4,.5,.05],[1.6,1.5,.84]]}
def request():
    return deepcopy(dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=[dict(op='short_path_command',id=k,case=k,actions=v)for k,v in CASES.items()]))
def validate(op):
    if not isinstance(op,dict)or set(op)!={'op','id','case','actions'}or op['op']!='short_path_command'or not isinstance(op['case'],str)or op['case']not in CASES or op['actions']!=CASES[op['case']]or not isinstance(op['id'],str)or re.match(r'\A[A-Za-z0-9_-]{1,80}\Z',op['id'])is None:raise ValueError('fixed owned ShortPath scenes required')
def validate_request(q):
    if type(q.get('iterations'))is not int or q['iterations']!=1 or not isinstance(q.get('operations'),list)or not 1<=len(q['operations'])<=4:raise ValueError('bounded ShortPath command request required')
    for op in q['operations']:validate(op)
    if len({op['id']for op in q['operations']})!=len(q['operations']):raise ValueError('unique ids required')
def run(op,host,iterations,tolerance):
    validate(op)
    from surface_short_path_probe import source
    from join_probe import observe_command
    from conic_getter_input import drive
    R=host['Rhino'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc
    if iterations!=1 or list(doc.Objects)or doc.UndoRecordingIsActive:raise ValueError('empty owned idle document required')
    surface=source('plane'if op['case']=='cancel'else op['case'],host)
    old=doc.Layers.CurrentLayerIndex;layers=[];identifier=None
    def snapshot():
        rows=[]
        for o in sorted(list(doc.Objects),key=lambda o:o.RuntimeSerialNumber):
            row=dict(role='target'if o.Id==identifier else'output',selected=bool(o.IsSelected(False)),layer=doc.Layers[o.Attributes.LayerIndex].Name,name=o.Attributes.Name,crc=int(o.Geometry.DataCRC(0)))
            if isinstance(o.Geometry,G.Curve):row['curve']=host['_nurbs_curve_definition'](o.Geometry)
            rows.append(row)
        return rows
    try:
        record=doc.BeginUndoRecord('Owned ShortPath source')
        try:
            for name in ['ShortPath source','ShortPath output']:
                layer=R.DocObjects.Layer();layer.Name=name;layers.append(doc.Layers.Add(layer))
            a=R.DocObjects.ObjectAttributes();a.LayerIndex=layers[0];a.Name='Target';identifier=doc.Objects.AddSurface(surface,a)
            doc.Layers.SetCurrentLayerIndex(layers[1],True);doc.Objects.Select(identifier)
        finally:doc.EndUndoRecord(record)
        doc.ClearUndoRecords(True);before=snapshot();traces=[]
        marker='Owned ShortPath '+str(host['System'].Guid.NewGuid());R.RhinoApp.WriteLine(marker)
        def execute():
            success,trace=drive(op,host,[identifier],'ShortPath');traces.append(trace);return success
        success,after,events=observe_command(R.Commands.Command,'ShortPath',execute,snapshot,lambda:[],True)
        after_script=snapshot();history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip()
        R.RhinoApp.RunScript('_Undo',False);undo=snapshot();R.RhinoApp.RunScript('_Redo',False);redo=snapshot()
        return dict(surface=host['_nurbs_surface_definition'](surface),success=success,before=before,after=after,after_script=after_script,undo=undo,redo=redo,input_trace=traces[0],events=events,history=history),0
    finally:
        doc.Layers.SetCurrentLayerIndex(old,True)
        for o in list(doc.Objects):doc.Objects.Delete(o.Id,True)
        for index in reversed(layers):doc.Layers.Delete(index,True)
        surface.Dispose()
