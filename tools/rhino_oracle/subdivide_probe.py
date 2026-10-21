"""Fixed public subdivision and source-state captures in an owned idle document."""
from copy import deepcopy
QUAD = [[0.,0.,0.],[4.,0.,0.],[4.,3.,0.],[0.,3.,0.]]
STRIP = [[0.,0.,0.],[2.,0.,0.],[4.,0.,1.],[0.,3.,0.],[2.,3.,0.],[4.,3.,1.]]

def source(kind='subd', vertices=None, faces=None, **kw):
    return dict(kind=kind,vertices=vertices or QUAD,faces=faces or [[0,1,2,3]],**kw)

def scene(kind='subd', **kw):
    return dict(sources=[source(kind)], **kw)

DISCOVERY = {
    'divide_subd_quad': scene(),
    'divide_mesh_quad': scene('mesh'),
    'divide_subd_tri': dict(sources=[source(faces=[[0,1,2]])]),
    'divide_mesh_tri': dict(sources=[source('mesh',faces=[[0,1,2]])]),
    'divide_subd_strip': dict(sources=[source(vertices=STRIP,faces=[[0,1,4,3],[1,2,5,4]])]),
    'divide_mesh_strip': dict(sources=[source('mesh',vertices=STRIP,faces=[[0,1,4,3],[1,2,5,4]])]),
    'divide_subd_keep': scene(delete=False),
    'divide_mesh_keep': scene('mesh',delete=False),
    'divide_subd_preselected': scene(preselected=True),
    'divide_mesh_preselected': scene('mesh',preselected=True),
    'divide_subd_local': dict(sources=[source(vertices=STRIP,faces=[[0,1,4,3],[1,2,5,4]],selected_faces=[0])]),
    'divide_mesh_local': dict(sources=[source('mesh',vertices=STRIP,faces=[[0,1,4,3],[1,2,5,4]],selected_faces=[0])]),
    'divide_subd_twice': scene(repeat=2),
    'divide_subd_number': scene(actions=[2]),
    'divide_mesh_number': scene('mesh',actions=[2]),
    'divide_cancel': scene(cancel=True),
}
CUBE = [[0.,0.,0.],[4.,0.,0.],[4.,3.,0.],[0.,3.,0.],[0.,0.,2.],[4.,0.,2.],[4.,3.,2.],[0.,3.,2.]]
CUBE_FACES = [[0,3,2,1],[0,1,5,4],[1,2,6,5],[2,3,7,6],[3,0,4,7],[4,5,6,7]]
SCENES = {}
for kind in ['subd','mesh']:
    def add(name, definition=None, **options):
        SCENES['subdivide_'+kind+'_'+name] = dict(sources=[definition or source(kind)], **options)
    add('quad'); add('triangle',source(kind,faces=[[0,1,2]]))
    add('preselected',preselected=True);add('twice',repeat=2);add('cancel',cancel=True)
    for name,vertices,faces in [('strip',STRIP,[[0,1,4,3],[1,2,5,4]]),('cube',CUBE,CUBE_FACES)]:
        add(name,source(kind,vertices=vertices,faces=faces))
        add(name+'_local',source(kind,vertices=vertices,faces=faces,selected_faces=[0]))
        add(name+'_all_faces',source(kind,vertices=vertices,faces=faces,selected_faces=list(range(len(faces)))))
    add('warped',source(kind,vertices=[[0.,0.,0.],[4.,0.,1.],[4.,3.,2.],[0.,3.,-1.]]))
    add('disconnected_local',source(kind,vertices=QUAD+[[x+8,y,z]for x,y,z in QUAD],faces=[[0,1,2,3],[4,5,6,7]],selected_faces=[1]))
    add('translated_triangle',source(kind,vertices=[[1e7,2e7,-3e7],[1e7+4,2e7,-3e7],[1e7+4,2e7+3,-3e7]],faces=[[0,1,2]]))
    add('mixed_polygon',source(kind,vertices=STRIP,faces=[[0,1,4,3],[1,2,4],[2,5,4]],selected_faces=[0,2]))
    SCENES['subdivide_'+kind+'_multiple'] = dict(sources=[source(kind),source(kind,vertices=[[x+8,y,z]for x,y,z in QUAD])])
SCENES['subdivide_mixed_objects'] = dict(sources=[source(),source('mesh',vertices=[[x+8,y,z]for x,y,z in QUAD])])
SCENES['subdivide_subd_corners'] = dict(sources=[source(corners=[0,1,2,3])])
SCENES['subdivide_subd_creased_cube'] = dict(sources=[source(vertices=CUBE,faces=CUBE_FACES,creases=[[0,1],[1,2],[2,3],[0,3]])])
SCENES['subdivide_mesh_colors'] = dict(sources=[source('mesh',colors=[[12,24,36,255],[100,0,0,255],[0,100,0,255],[0,0,100,255]])])
SCENES['subdivide_mesh_ngon'] = dict(sources=[source('mesh',vertices=STRIP,faces=[[0,1,4,3],[1,2,5,4]],ngons=[dict(vertices=[0,1,2,5,4,3],faces=[0,1])])])
SEAM = [[0.,0.,0.],[2.,0.,0.],[2.,3.,0.],[0.,3.,0.],[2.,0.,0.],[4.,0.,1.],[4.,3.,1.],[2.,3.,0.]]
for local in [False,True]:
    suffix='_local'if local else''
    SCENES['subdivide_mesh_seam'+suffix]=dict(sources=[source('mesh',vertices=SEAM,faces=[[0,1,2,3],[4,5,6,7]],selected_faces=[0]if local else[])])
    SCENES['subdivide_mesh_nonmanifold'+suffix]=dict(sources=[source('mesh',vertices=[[0.,0.,0.],[4.,0.,0.],[0.,3.,0.],[0.,-3.,0.],[0.,0.,3.]],faces=[[0,1,2],[1,0,3],[0,1,4]],selected_faces=[0]if local else[])])


DETAILS = {}
def star(vertices, masks):
    vertices=deepcopy(vertices);faces=[list(range(len(vertices)))];n=len(vertices)
    for i in range(n):
        a=i;b=(i+1)%n;pa=vertices[a];pb=vertices[b];dx=(pb[1]-pa[1])*0.5;dy=-(pb[0]-pa[0])*0.5
        aa=len(vertices);vertices.append([pa[0]+dx,pa[1]+dy,pa[2]])
        bb=len(vertices);vertices.append([pb[0]+dx,pb[1]+dy,pb[2]])
        faces.append([b,a,aa,bb])
    return source('mesh',vertices=vertices,faces=faces,selected_faces=[i+1 for i in masks])
for polygon,vertices,masks in [
    ('tri',[[0.,0.,0.],[4.,0.,0.],[0.,4.,0.]],[[0],[0,1],[0,1,2]]),
    ('quad',[[0.,0.,0.],[4.,0.,0.],[4.,4.,0.],[0.,4.,0.]],[[0],[0,1],[0,2],[0,1,2],[0,1,2,3]]),
    ('warped',[[0.,0.,0.],[4.,0.,1.],[4.,4.,2.],[0.,4.,-1.]],[[0],[0,1],[0,2],[0,1,2],[0,1,2,3]])]:
    for mask in masks:
        DETAILS['subdivide_neighbor_'+polygon+'_'.join(['']+[str(i)for i in mask])]=dict(sources=[star(vertices,mask)])
DETAILS['subdivide_subd_local_twice']=dict(sources=[source(vertices=STRIP,faces=[[0,1,4,3],[1,2,5,4]],selected_faces=[0])],repeat=2,reselect_children=True)
for name,selected in [('raw_ngon',[0]),('whole_ngon',[0,1])]:
    DETAILS['subdivide_mesh_'+name]=dict(sources=[source('mesh',vertices=STRIP,faces=[[0,1,4,3],[1,2,5,4]],ngons=[dict(vertices=[0,1,2,5,4,3],faces=[0,1])],selected_faces=selected)])
DETAILS['subdivide_mesh_odd_colors']=dict(sources=[source('mesh',vertices=QUAD[:3],faces=[[0,1,2]],colors=[[1,3,5,7],[2,4,6,8],[254,252,250,248]])])


ORIENTATIONS = {}
for polygon,vertices in [('tri',[[0.,0.,0.],[4.,0.,0.],[0.,4.,0.]]),('quad',[[0.,0.,0.],[4.,0.,0.],[4.,4.,0.],[0.,4.,0.]]),('warped',[[0.,0.,0.],[4.,0.,1.],[4.,4.,2.],[0.,4.,-1.]])]:
    for mask in range(1,1<<len(vertices)):
        for reverse in [False,True]:
            definition=star(vertices,[i for i in range(len(vertices))if mask&(1<<i)])
            if reverse:definition['faces']=[list(reversed(f))for f in definition['faces']]
            ORIENTATIONS['subdivide_mask_'+polygon+'_'+str(mask)+('_reverse'if reverse else'')]=dict(sources=[definition])


EXTRAS={}
base=source('mesh',vertices=STRIP+[[x+10,y,z]for x,y,z in STRIP],faces=[[0,1,4,3],[1,2,5,4],[6,7,10,9],[7,8,11,10]],ngons=[dict(vertices=[0,1,2,5,4,3],faces=[0,1]),dict(vertices=[6,7,8,11,10,9],faces=[2,3])])
for name,selected in [('partial',[0]),('whole',[0,1])]:
    definition=deepcopy(base);definition['selected_faces']=selected
    EXTRAS['subdivide_unrelated_ngon_'+name]=dict(sources=[definition])
EXTRAS['subdivide_neighbor_existing_ngon']=dict(sources=[source('mesh',vertices=[[0.,0.,0.],[2.,0.,0.],[4.,0.,0.],[6.,0.,0.],[0.,3.,0.],[2.,3.,0.],[4.,3.,0.],[6.,3.,0.]],faces=[[0,1,5,4],[1,2,6,5],[2,3,7,6]],ngons=[dict(vertices=[1,2,3,7,6,5],faces=[1,2])],selected_faces=[0])])
EXTRAS['subdivide_mesh_seam_twice']=dict(sources=[source('mesh',vertices=SEAM,faces=[[0,1,2,3],[4,5,6,7]])],repeat=2)


NGONS={}
vertices=[[float(x),float(y),0.]for y in [0,3]for x in [0,2,4,6,8]]
faces=[[i,i+1,i+6,i+5]for i in range(4)]
for name,picked in [('first',[0]),('middle',[1]),('ends',[0,3]),('two_middle',[1,2]),('all',[0,1,2,3])]:
    NGONS['subdivide_ngon_members_'+name]=dict(sources=[source('mesh',vertices=vertices,faces=faces,ngons=[dict(vertices=[0,1,2,3,4,9,8,7,6,5],faces=[0,1,2,3])],selected_faces=picked)])

def all_cases():
    result={}
    for cases in [DISCOVERY,SCENES,DETAILS,ORIENTATIONS,EXTRAS,NGONS]:result.update(cases)
    return result

def request(discovery=False, details=False, orientations=False, extra=False, ngons=False):
    return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=[dict(op='subdivide_command',id=k,case=k)for k in (DISCOVERY if discovery else DETAILS if details else ORIENTATIONS if orientations else EXTRAS if extra else NGONS if ngons else SCENES)])

def validate(op):
    cases=all_cases()
    try: valid=isinstance(op,dict)and set(op)=={'op','id','case'}and op['op']=='subdivide_command'and op['case']in cases and op['id']==op['case']
    except TypeError: valid=False
    if not valid:raise ValueError('fixed subdivision recipe required')

def run(op,host,iterations,tolerance):
    validate(op)
    from join_probe import observe_command
    from subd_box_probe import subd_value
    R,S=host['Rhino'],host['System'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc
    if iterations!=1 or list(doc.Objects)or doc.UndoRecordingIsActive:raise ValueError('empty owned idle document required')
    case=deepcopy(all_cases()[op['case']]);ids=[];layers=[];group=-1
    def faces(g):return list(g.Faces)
    def snapshot():
        result=[]
        for obj in sorted(list(doc.Objects),key=lambda o:o.RuntimeSerialNumber):
            g=obj.Geometry;a=obj.Attributes
            row=dict(source=ids.index(obj.Id)if obj.Id in ids else None,selected=bool(obj.IsSelected(False)),name=a.Name,layer=doc.Layers[a.LayerIndex].Name,groups=[doc.Groups.GroupName(i)for i in (a.GetGroupList()or[])],color=[int(a.ObjectColor.R),int(a.ObjectColor.G),int(a.ObjectColor.B)],color_source=int(a.ColorSource),crc=int(g.DataCRC(0)))
            if isinstance(g,G.SubD):
                row['subd']=subd_value(g,host);row['selected_faces']=[i for i,f in enumerate(faces(g))if obj.IsSubObjectSelected(f.ComponentIndex())]
            elif isinstance(g,G.Mesh):
                row['mesh']=host['_polygon_mesh_value'](g);row['mesh']['colors']=[[int(c.R),int(c.G),int(c.B),int(c.A)]for c in g.VertexColors]if g.VertexColors.Count else None;row['mesh']['ngons']=[dict(vertices=[int(i)for i in n.BoundaryVertexIndexList()],faces=[int(i)for i in n.FaceIndexList()])for n in g.Ngons];row['selected_faces']=[i for i in range(g.Faces.Count)if obj.IsSubObjectSelected(G.ComponentIndex(G.ComponentIndexType.MeshFace,i))]
            else:row['point']=host['_xyz'](g.Location)
            result.append(row)
        return result
    try:
        record=doc.BeginUndoRecord('Owned subdivision setup')
        try:
            for name,color in [('Subdivision input',S.Drawing.Color.Blue),('Subdivision current',S.Drawing.Color.Green)]:layers.append(doc.Layers.Add(name,color))
            group=doc.Groups.Add('Subdivision assembly')
            for index,definition in enumerate(case['sources']):
                m=G.Mesh();m.Vertices.UseDoublePrecisionVertices=True
                for p in definition['vertices']:m.Vertices.Add(G.Point3d(*p))
                for f in definition['faces']:m.Faces.AddFace(*f)
                for color in definition.get('colors',[]):m.VertexColors.Add(S.Drawing.Color.FromArgb(color[3],color[0],color[1],color[2]))
                for ngon in definition.get('ngons',[]):m.Ngons.AddNgon(G.MeshNgon.Create(S.Array[S.UInt32](ngon['vertices']),S.Array[S.UInt32](ngon['faces'])))
                m.Normals.ComputeNormals()
                a=R.DocObjects.ObjectAttributes();a.LayerIndex=layers[0];a.Name='Input '+str(index);a.ObjectColor=S.Drawing.Color.FromArgb(17,43,91);a.ColorSource=R.DocObjects.ObjectColorSource.ColorFromObject;a.AddToGroup(group)
                if definition['kind']=='subd':
                    g=G.SubD.CreateFromMesh(m)
                    if g is None:raise ValueError('cannot construct source SubD')
                    vertices=[];v=g.Vertices.First
                    while v is not None:vertices.append(v);v=v.Next
                    for i in definition.get('corners',[]):vertices[i].Tag=G.SubDVertexTag.Corner
                    vertex_index={int(v.Id):i for i,v in enumerate(vertices)}
                    for edge in g.Edges:
                        if sorted([vertex_index[int(edge.VertexFrom.Id)],vertex_index[int(edge.VertexTo.Id)]])in definition.get('creases',[]):edge.Tag=G.SubDEdgeTag.Crease
                    g.UpdateAllTagsAndSectorCoefficients();key=doc.Objects.AddSubD(g,a);g.Dispose()
                else:key=doc.Objects.AddMesh(m,a)
                m.Dispose();ids.append(key)
                obj=doc.Objects.FindId(key)
                if definition.get('selected_faces'):
                    for i in definition['selected_faces']:
                        component=faces(obj.Geometry)[i].ComponentIndex()if definition['kind']=='subd'else G.ComponentIndex(G.ComponentIndexType.MeshFace,i)
                        if not obj.SelectSubObject(component,True,True,False):raise ValueError('component preselection failed')
                elif case.get('preselected'):doc.Objects.Select(key)
            doc.Layers.SetCurrentLayerIndex(layers[1],True)
        finally:doc.EndUndoRecord(record)
        doc.ClearUndoRecords(True);before=snapshot();macros=[];steps=[]
        for step in range(case.get('repeat',1)):
            if step and case.get('reselect_children'):
                for row in steps[-1]['after']:
                    obj=doc.Objects.FindId(ids[row['source']])
                    for i in row['selected_faces']:
                        if not obj.SelectSubObject(faces(obj.Geometry)[i].ComponentIndex(),True,True,False):raise ValueError('child reselection failed')
            macro='_-SubDivide'
            if not case.get('preselected')and not any(d.get('selected_faces')for d in case['sources']):
                if op['case']in DISCOVERY:macro+=' _DeleteInput='+('Yes'if case.get('delete',True)else'No')
                for key in ids:macro+=" '_-SelID "+str(key)
            for a in case.get('actions',[]):macro+=' '+str(a)
            macro+=' _Cancel'if case.get('cancel')else' _Enter'
            macro+=' _Cancel _Cancel _Cancel';macros.append(macro)
            marker='Owned subdivision '+str(S.Guid.NewGuid());R.RhinoApp.WriteLine(marker)
            success,after,events=observe_command(R.Commands.Command,'SubDivide',lambda:R.RhinoApp.RunScript(macro,True),snapshot,lambda:[],True)
            history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip()
            steps.append(dict(success=success,events=events,history=history,after=after))
        after_script=snapshot();R.RhinoApp.RunScript('_Undo',False);undo=snapshot();R.RhinoApp.RunScript('_Redo',False);redo=snapshot()
        return dict(input=case,macros=macros,before=before,steps=steps,after_script=after_script,undo=undo,redo=redo),0
    finally:
        doc.Layers.SetCurrentLayerIndex(0,True)
        for obj in list(doc.Objects):doc.Objects.Delete(obj.Id,True)
        if group>=0:doc.Groups.Delete(group)
        for layer in reversed(layers):doc.Layers.Delete(layer,True)
