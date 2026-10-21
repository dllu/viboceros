"""Fixed public line-network meshing inputs in an owned idle document."""
from copy import deepcopy
FROZEN_HEIGHTS = {29: 2.217236205811076,
 30: 2.309401076758503,
 31: 2.4034424761102415,
 34: 2.698034067369707,
 35: 2.800830152838839,
 36: 2.906170112021443,
 45: 3.9999999999999996,
 46: 4.142121255162278,
 50: 4.76701437037684,
 55: 5.712592026968458,
 59: 6.657117929402073,
 60: 6.928203230275507,
 61: 7.216191021085694,
 63: 7.8504420220206015}


def polygon(points):
    return [[deepcopy(a), deepcopy(b)] for a, b in zip(points, points[1:] + points[:1])]


TRIANGLE = polygon([[0., 0., 0.], [4., 0., 0.], [0., 3., 0.]])
QUAD = polygon([[0., 0., 0.], [4., 0., 0.], [4., 3., 0.], [0., 3., 0.]])
PENTAGON = polygon([[0., 0., 0.], [3., 0., 0.], [4., 2., 0.], [2., 4., 0.], [-1., 2., 0.]])
CASES = {
    'triangle': dict(lines=deepcopy(TRIANGLE)),
    'quad': dict(lines=deepcopy(QUAD)),
    'quad_three': dict(lines=deepcopy(QUAD), max_sides=3),
    'quad_diagonal': dict(lines=deepcopy(QUAD)+[[[0.,0.,0.],[4.,3.,0.]]]),
    'quad_crossing': dict(lines=deepcopy(QUAD)+[[[0.,0.,0.],[4.,3.,0.]],[[4.,0.,0.],[0.,3.,0.]]]),
    'pentagon_four': dict(lines=deepcopy(PENTAGON)),
    'pentagon_five': dict(lines=deepcopy(PENTAGON), max_sides=5),
    'open': dict(lines=deepcopy(QUAD[:3])),
    'dangling': dict(lines=deepcopy(QUAD)+[[[0.,0.,0.],[-2.,-1.,0.]]]),
    'duplicate': dict(lines=deepcopy(QUAD)+[deepcopy(QUAD[0]),list(reversed(deepcopy(QUAD[1])))]),
    'two_regions': dict(lines=deepcopy(QUAD)+polygon([[4.,0.,0.],[8.,0.,0.],[8.,3.,0.],[4.,3.,0.]])),
    'disconnected': dict(lines=deepcopy(TRIANGLE)+polygon([[10.,0.,1.],[14.,0.,1.],[10.,3.,1.]])),
    'delete': dict(lines=deepcopy(QUAD), delete=True),
    'current_layer': dict(lines=deepcopy(QUAD), current_layer=True),
    'near_endpoint': dict(lines=deepcopy(QUAD)),
    'far_endpoint': dict(lines=deepcopy(QUAD)),
}
CASES['near_endpoint']['lines'][1][0][0]+=2e-7
CASES['near_endpoint']['lines'][0][1][0]+=2e-7
CASES['far_endpoint']['lines'][1][0][0]+=2e-6
CASES['far_endpoint']['lines'][0][1][0]+=2e-6
_CUBE=[[float(x),float(y),float(z)]for z in [0,3]for y in [0,3]for x in [0,3]]
CASES['cube']=dict(lines=[[_CUBE[i],_CUBE[j]]for i in range(8)for j in range(i+1,8)if sum(a!=b for a,b in zip(_CUBE[i],_CUBE[j]))==1])
_TETRA=[[0.,0.,0.],[4.,0.,0.],[0.,3.,0.],[0.,0.,2.]]
CASES['tetrahedron']=dict(lines=[[_TETRA[i],_TETRA[j]]for i in range(4)for j in range(i+1,4)])
CASES['current_layer']=CASES.pop('current_layer')
VARIANTS={}
for _name,_gap in [('gap_20u',2e-5),('gap_200u',2e-4),('gap_2m',2e-3),('gap_20m',2e-2)]:
    VARIANTS[_name]=dict(lines=deepcopy(QUAD));VARIANTS[_name]['lines'][1][0][0]+=_gap;VARIANTS[_name]['lines'][0][1][0]+=_gap
for _name,_tol in [('sdk_tight',1e-12),('sdk_loose',.1)]:
    VARIANTS[_name]=dict(deepcopy(CASES['far_endpoint']),sdk_tolerance=_tol)
VARIANTS['cube_order']=dict(lines=list(reversed(deepcopy(CASES['cube']['lines']))))
VARIANTS['cube_directions']=dict(lines=[list(reversed(line))for line in deepcopy(CASES['cube']['lines'])])
VARIANTS['cube_six']=dict(deepcopy(CASES['cube']),max_sides=6)
VARIANTS['hexagon']=dict(lines=polygon([[0.,0.,0.],[3.,0.,0.],[4.,2.,0.],[3.,4.,0.],[0.,4.,0.],[-1.,2.,0.]]),max_sides=6)
VARIANTS['concave']=dict(lines=polygon([[0.,0.,0.],[4.,0.,0.],[4.,4.,0.],[2.,1.,0.],[0.,4.,0.]]),max_sides=5)
VARIANTS['input_layer']=dict(lines=deepcopy(QUAD),output_layer='Input')
VARIANTS['output_layer']=dict(lines=deepcopy(QUAD),output_layer='Current')
for _name,_height in [('shallow',.01),('fold',1.),('flat',0.)]:
    VARIANTS[_name]=dict(lines=deepcopy(QUAD)+polygon([[4.,0.,0.],[8.,0.,_height],[8.,3.,_height],[4.,3.,0.]]))
SCENES=CASES.copy();SCENES.update(VARIANTS)
CONTROLS={}
for _name,_gap,_tol in [('actual_gap_small',2e-7,None),('actual_gap_equal',1e-6,None),('actual_gap_large',2e-6,None),('actual_gap_huge',.02,None),('actual_sdk_tight',2e-7,1e-12),('actual_sdk_loose',.02,.1)]:
    _case=dict(lines=deepcopy(QUAD));_case['lines'][1][0]=[4.+_gap,0.,0.]
    if _tol is not None:_case['sdk_tolerance']=_tol
    CONTROLS[_name]=_case
CONTROLS['transitive']=deepcopy(CONTROLS['actual_gap_small']);CONTROLS['transitive']['lines'][1][0]=[4.+8e-7,0.,0.];CONTROLS['transitive']['lines'].append([[4.+16e-7,0.,0.],[7.,0.,0.]])
for _degree in [29,30,31,34,35,36,45,63]:
    _height=FROZEN_HEIGHTS[_degree]
    CONTROLS['fold_'+str(_degree)]=dict(lines=deepcopy(QUAD)+polygon([[4.,0.,0.],[8.,0.,_height],[8.,3.,_height],[4.,3.,0.]]))
CONTROLS['source_layer']=dict(lines=deepcopy(QUAD),output_layer='MeshFromLines source')
CONTROLS['restore_current']=dict(lines=deepcopy(QUAD),output_layer='Current')
CONTROLS['mixed_polyline']=dict(lines=deepcopy(TRIANGLE),polylines=[[[10.,0.,0.],[13.,0.,0.],[13.,3.,0.],[10.,3.,0.],[10.,0.,0.]]])
SCENES.update(CONTROLS)
EDGE_CASES={}
for _degree in [46,50,55,59,60,61]:
    _height=FROZEN_HEIGHTS[_degree]
    EDGE_CASES['crease_'+str(_degree)]=dict(lines=deepcopy(QUAD)+polygon([[4.,0.,0.],[8.,0.,_height],[8.,3.,_height],[4.,3.,0.]]),reset_layer=True)
EDGE_CASES['layer_index']=dict(lines=deepcopy(QUAD),output_layer='SourceIndex',reset_layer=True)
EDGE_CASES['layer_input_name']=dict(lines=deepcopy(QUAD),output_layer='_InputLayer',reset_layer=True)
SCENES.update(EDGE_CASES)


def request():
    return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=[dict(op='mesh_from_lines_command',id=name,case=name)for name in CASES])

def variant_request():
    q=request();q['operations']=[dict(op='mesh_from_lines_command',id=name,case=name)for name in VARIANTS];return q
def control_request():
    q=request();q['operations']=[dict(op='mesh_from_lines_command',id=name,case=name)for name in CONTROLS];return q
def edge_request():
    q=request();q['operations']=[dict(op='mesh_from_lines_command',id=name,case=name)for name in EDGE_CASES];return q


def validate(op):
    try:
        valid=isinstance(op,dict)and set(op)=={'op','id','case'}and op['op']=='mesh_from_lines_command'and op['case']in SCENES and op['id']==op['case']
    except TypeError:
        valid=False
    if not valid:raise ValueError('fixed MeshFromLines case required')


def run(op,host,iterations,tolerance):
    validate(op)
    from join_probe import observe_command
    R=host['Rhino'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc
    if iterations!=1 or list(doc.Objects)or doc.UndoRecordingIsActive:
        raise ValueError('empty owned idle document required')
    case=deepcopy(SCENES[op['case']]);curves=[G.LineCurve(G.Point3d(*a),G.Point3d(*b))for a,b in case['lines']]
    curves.extend(G.PolylineCurve([G.Point3d(*p)for p in polyline])for polyline in case.get('polylines',[]))
    sources=[];old_layer=doc.Layers.CurrentLayerIndex;layers=[]
    def snapshot():
        rows=[]
        for obj in sorted(list(doc.Objects),key=lambda o:o.RuntimeSerialNumber):
            row=dict(role='source'if obj.Id in sources else'output',selected=bool(obj.IsSelected(False)),layer=doc.Layers[obj.Attributes.LayerIndex].Name,name=obj.Attributes.Name,crc=int(obj.Geometry.DataCRC(0)))
            if isinstance(obj.Geometry,G.Mesh):row['mesh']=host['_polygon_mesh_value'](obj.Geometry)
            if isinstance(obj.Geometry,G.Curve):row['ends']=[host['_xyz'](obj.Geometry.PointAtStart),host['_xyz'](obj.Geometry.PointAtEnd)]
            rows.append(row)
        return rows
    try:
        crc_before=[int(c.DataCRC(0))for c in curves]
        sdk=G.Mesh.CreateFromLines(host['System'].Array[G.Curve](curves),case.get('max_sides',4),case.get('sdk_tolerance',tolerance['absolute']))
        try:sdk_value=host['_polygon_mesh_value'](sdk)if sdk else None
        finally:
            if sdk:sdk.Dispose()
        crc_after=[int(c.DataCRC(0))for c in curves]
        record=doc.BeginUndoRecord('Owned MeshFromLines source')
        try:
            for name in ['MeshFromLines source','MeshFromLines output']:
                layer=R.DocObjects.Layer();layer.Name=name;layers.append(doc.Layers.Add(layer))
            attributes=R.DocObjects.ObjectAttributes();attributes.LayerIndex=layers[0];attributes.Name='Input'
            for curve in curves:sources.append(doc.Objects.AddCurve(curve,attributes))
            doc.Layers.SetCurrentLayerIndex(layers[1],True)
            for identifier in sources:doc.Objects.Select(identifier)
        finally:doc.EndUndoRecord(record)
        doc.ClearUndoRecords(True);before=snapshot()
        marker='Owned MeshFromLines '+str(host['System'].Guid.NewGuid());R.RhinoApp.WriteLine(marker)
        command='_-MeshFromLines'
        if case.get('reset_layer'):command+=' _OutputLayer=Current'
        command+=' _MaxSidesPerFace='+str(case.get('max_sides',4))
        command+=' _DeleteInput='+('Yes'if case.get('delete')else'No')
        if 'output_layer'in case:
            target=str(layers[0])if case['output_layer']=='SourceIndex'else case['output_layer']
            command+=' _OutputLayer='+('"'+target+'"'if' 'in target else target)
        if case.get('current_layer'):command+=' _CurrentLayer'
        command+=' _Enter'
        success,after,events=observe_command(R.Commands.Command,'MeshFromLines',lambda:R.RhinoApp.RunScript(command,True),snapshot,lambda:[],True)
        history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip();after_script=snapshot()
        R.RhinoApp.RunScript('_Undo',False);undo=snapshot();R.RhinoApp.RunScript('_Redo',False);redo=snapshot()
        return dict(input=case,sdk=sdk_value,sdk_inputs_unchanged=crc_before==crc_after,requested_tolerance=tolerance,document_tolerance=doc.ModelAbsoluteTolerance,sdk_signature=str(G.Mesh.CreateFromLines.__doc__),success=success,events=events,history=history,before=before,after=after,after_script=after_script,undo=undo,redo=redo),0
    finally:
        doc.Layers.SetCurrentLayerIndex(old_layer,True)
        for obj in list(doc.Objects):doc.Objects.Delete(obj.Id,True)
        for layer in reversed(layers):doc.Layers.Delete(layer,True)
        for curve in curves:curve.Dispose()
