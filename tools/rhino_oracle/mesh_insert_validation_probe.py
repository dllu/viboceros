"""Public Native validity checks on explicitly output-derived calibration meshes."""
import json,os
NAMES=('flat-edge-0','flat-edge-1','flat-edge-3','flat-three','flat-reverse','flat-same-high',
       'warped-edge-0','warped-edge-1','warped-three','warped-reverse','warped-same-low',
       'warped-same-high','warped-reversed-face','warped-cyclic-face','skewed-flat','tall-warp','triangle','triangle-ngon')
def validate(op):
    if not(isinstance(op,dict)and set(op)=={'op','id','case'}and op['op']in('mesh_insert_validation','mesh_insert_conversion')
           and isinstance(op['case'],str)and op['case']in NAMES and op['case']==op['id']):
        raise ValueError('fixed public mesh validation calibration required')
def run(op,host,iterations,tolerance):
    validate(op)
    if iterations!=1 or list(host['Rhino'].RhinoDoc.ActiveDoc.Objects):raise ValueError('owned empty idle validation required')
    with open(os.path.join(os.path.dirname(__file__),'fixtures','mesh_insert_validation_assets.json'))as f:c=json.load(f)[op['case']]
    G=host['Rhino'].Geometry;m=G.Mesh();m.Vertices.UseDoublePrecisionVertices=True
    try:
        for p in c['vertices']:m.Vertices.Add(*p)
        for f in c['faces']:m.Faces.AddFace(*f)
        valid=bool(m.IsValid)
        converted=bool(m.Faces.ConvertQuadsToTriangles())if op['op']=='mesh_insert_conversion'else None
        try:log=m.IsValidWithLog()
        except Exception as error:log=[str(type(error).__name__)]
        return dict(valid=valid,converted=converted,result_valid=bool(m.IsValid),validity_log=[str(x)for x in log],mesh=host['_polygon_mesh_value'](m)),0
    finally:m.Dispose()
