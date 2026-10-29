"""Observe public MergeFaces on an owned mesh; keep complete stored channels."""
import math
import re


def validate(op):
    if (not isinstance(op,dict) or set(op)!={'op','id','vertices','faces','components','channels','ngons'}
            or op['op']!='mesh_merge_faces' or not isinstance(op['id'],str)
            or re.match(r'^[A-Za-z0-9_-]{1,80}\Z',op['id']) is None
            or op['channels'] not in ['none','all']):
        raise ValueError('fixed bounded merge recipe required')
    vertices,faces,components,ngons=op['vertices'],op['faces'],op['components'],op['ngons']
    if not isinstance(vertices,list) or not 3<=len(vertices)<=128 or not isinstance(faces,list) or not 1<=len(faces)<=128:
        raise ValueError('bounded mesh required')
    if any(not isinstance(p,list) or len(p)!=3 or any(type(x)not in(int,float) or math.isnan(x)or math.isinf(x)for x in p)for p in vertices):
        raise ValueError('finite mesh vertices required')
    if any(not isinstance(f,list) or len(f)not in[3,4] or any(type(i)is not int or not 0<=i<len(vertices)for i in f) or len(set(f))!=len(f)for f in faces):
        raise ValueError('valid mesh faces required')
    if not isinstance(components,list) or len(components)>128 or not isinstance(ngons,list) or len(ngons)>len(faces):
        raise ValueError('bounded mesh components required')
    for c in components:
        if not isinstance(c,dict) or c.get('kind')not in['face','vertex','edge']:
            raise ValueError('typed mesh component required')
        if c['kind']=='edge':
            if set(c)!={'kind','vertices'} or not isinstance(c['vertices'],list) or len(c['vertices'])!=2 or any(type(i)is not int or not 0<=i<len(vertices)for i in c['vertices']):
                raise ValueError('raw edge endpoints required')
        elif set(c)!={'kind','index'} or type(c['index'])is not int or not 0<=c['index']<(len(faces)if c['kind']=='face'else len(vertices)):
            raise ValueError('bounded component index required')
    for g in ngons:
        if not isinstance(g,dict) or set(g)!={'vertices','faces'} or not isinstance(g['vertices'],list) or not 3<=len(g['vertices'])<=len(vertices) or not isinstance(g['faces'],list) or not 1<=len(g['faces'])<=len(faces):
            raise ValueError('bounded n-gon required')
        if any(type(i)is not int or not 0<=i<len(vertices)for i in g['vertices'])or any(type(i)is not int or not 0<=i<len(faces)for i in g['faces']):
            raise ValueError('bounded n-gon indices required')


def run(op,host,iterations,tolerance):
    validate(op)
    R,S=host['Rhino'],host['System'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc
    if iterations!=1 or list(doc.Objects) or R.Commands.Command.InCommand():
        raise ValueError('empty owned idle document required')
    def value(mesh):
        if mesh is None:return None
        result=host['_polygon_mesh_value'](mesh)
        result.update(colors=[[int(c.R),int(c.G),int(c.B),int(c.A)]for c in mesh.VertexColors],
                      uv=[[float(p.X),float(p.Y)]for p in mesh.TextureCoordinates],
                      normals=[[float(n.X),float(n.Y),float(n.Z)]for n in mesh.Normals],
                      ngons=[dict(vertices=[int(i)for i in n.BoundaryVertexIndexList()],faces=[int(i)for i in n.FaceIndexList()])for n in mesh.Ngons],valid=bool(mesh.IsValid))
        return result
    mesh=G.Mesh()
    try:
        for p in op['vertices']:mesh.Vertices.Add(G.Point3d(*p))
        for f in op['faces']:mesh.Faces.AddFace(*f)
        for g in op['ngons']:
            mesh.Ngons.AddNgon(G.MeshNgon.Create(S.Array[S.UInt32](g['vertices']),S.Array[S.UInt32](g['faces'])))
        if op['channels']=='all':
            for i in range(mesh.Vertices.Count):
                mesh.TextureCoordinates.Add(float(i),float(i*i))
                mesh.VertexColors.Add(S.Drawing.Color.FromArgb(255,17*i%256,31*i%256,47*i%256))
            mesh.Normals.ComputeNormals()
        mesh.Vertices.UseDoublePrecisionVertices=True
        if not mesh.IsValid:raise ValueError('invalid authored source')
        attr=R.DocObjects.ObjectAttributes();attr.Name='Merge source'
        try:key=doc.Objects.AddMesh(mesh,attr)
        finally:attr.Dispose()
        if key==S.Guid.Empty:raise ValueError('owned mesh insertion failed')
        doc.ClearUndoRecords(True);doc.ClearRedoRecords()
        obj=doc.Objects.FindId(key);before=value(obj.Geometry)
        selected=[]
        for c in op['components']:
            if c['kind']=='edge':
                a,b=[obj.Geometry.TopologyVertices.TopologyVertexIndex(i)for i in c['vertices']]
                index=obj.Geometry.TopologyEdges.GetEdgeIndex(a,b)
                if index<0:raise ValueError('selected endpoints do not form an edge')
                kind=G.ComponentIndexType.MeshTopologyEdge
            else:
                index=c['index'];kind=G.ComponentIndexType.MeshFace if c['kind']=='face'else G.ComponentIndexType.MeshVertex
            if not obj.SelectSubObject(G.ComponentIndex(kind,index),True,True,False):raise ValueError('owned component selection failed')
            selected.append(dict(kind=c['kind'],index=int(index)))
        events=[]
        def ended(sender,event):
            if event.CommandEnglishName=='MergeFaces':events.append(str(event.CommandResult))
        marker='Owned MergeFaces '+str(S.Guid.NewGuid());R.RhinoApp.WriteLine(marker)
        R.Commands.Command.EndCommand+=ended
        try:
            from hide_pt_input import run as drive
            _,getter,_=drive(host,command='MergeFaces',cancel=True)
        finally:R.Commands.Command.EndCommand-=ended
        if len(events)!=1:raise ValueError('merge did not finish: '+str(events))
        objects=list(doc.Objects)
        if len(objects)>1:raise ValueError('unexpected merge outputs')
        after=value(objects[0].Geometry)if objects else None
        selected_after=[dict(kind=str(c.ComponentIndexType),index=int(c.Index))for c in (objects[0].GetSelectedSubObjects()or[])]if objects else[]
        selected_owner=bool(objects[0].IsSelected(False))if objects else False
        attributes=not objects or objects[0].Attributes.Name=='Merge source'
        history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[-1]
        R.RhinoApp.RunScript('_Undo',False)
        objects=list(doc.Objects);undone=value(objects[0].Geometry)if objects else None
        R.RhinoApp.RunScript('_Redo',False)
        objects=list(doc.Objects);redone=value(objects[0].Geometry)if objects else None
        # Historical field name: this is whether the bounded input timer fired,
        # not proof that a transient getter did or did not open.
        return dict(before=before,after=after,undone=undone,redone=redone,attributes_preserved=attributes,
                    selected_input=selected,selected_after=selected_after,selected_owner=selected_owner,
                    command_result=events[0],getter_opened=getter,history=history),0
    finally:
        mesh.Dispose();doc.Objects.UnselectAll()
        for obj in list(doc.Objects):doc.Objects.Delete(obj.Id,True)
