"""Owned block-leaf mouse picks followed by the public Reparameterize command."""
import math
import re
if __package__:
    from .block_workflow_probe import validate as validate_blocks, _numbers
else:
    from block_workflow_probe import validate as validate_blocks, _numbers

CURVES = ('line', 'circle', 'nurbs')
SURFACES = ('annulus', 'cylinder', 'sphere', 'box')


def validate(op):
    if (not isinstance(op, dict) or set(op)-{'display','selection_choice','selection_choices','alternate_family','alternate_domain'} != {'op', 'id', 'family', 'placements', 'member_path', 'actions', 'view', 'artifact_path'}
            or op['op'] != 'reparameterize_block_command' or op['family'] not in CURVES+SURFACES
            or not isinstance(op['id'], str) or re.match(r'^[A-Za-z0-9_-]{1,100}\Z', op['id']) is None
            or op['view'] not in ('Top', 'Front', 'Right', 'Perspective')
            or not isinstance(op['artifact_path'], str) or not op['artifact_path'].endswith('.3dm')):
        raise ValueError('invalid block reparameterization recipe')
    if op.get('display','Shaded') not in ('Shaded','Wireframe'):
        raise ValueError('invalid block pick display')
    if 'selection_choice' in op and (type(op['selection_choice']) is not int or not 0<=op['selection_choice']<32):
        raise ValueError('invalid explicit overlap choice')
    if op.get('alternate_family',op['family']) not in CURVES+SURFACES:
        raise ValueError('invalid alternate family')
    if 'alternate_domain' in op:
        _numbers(op['alternate_domain'],2)
        if op.get('alternate_family',op['family']) not in CURVES or op['alternate_domain'][0]>=op['alternate_domain'][1]:
            raise ValueError('invalid alternate curve domain')
    placements, path = op['placements'], op['member_path']
    if (not isinstance(placements, list) or not 1 <= len(placements) <= 3
            or not isinstance(path, list) or len(path) != len(placements)
            or any(type(i) is not int or i not in (0, 1) for i in path)):
        raise ValueError('invalid bounded block member path')
    # Share the established affine admission validator, including its exact
    # nonsingularity check, rather than introducing another matrix policy.
    validate_blocks(dict(sources=[dict(type='line')], steps=[dict(action='create', name='source',
        base=[0., 0., 0.], sources=[0])]+[dict(action='insert', name='source', transform=m) for m in placements]))
    actions = op['actions']
    if (not isinstance(actions, list) or not 2 <= len(actions) <= 12
            or not isinstance(actions[0], dict) or set(actions[0]) != {'click'}):
        raise ValueError('bounded mouse picks and domain inputs are required')
    count = 0
    while count < len(actions) and isinstance(actions[count], dict):
        if set(actions[count]) != {'click'} or count >= 3:
            raise ValueError('invalid bounded mouse pick')
        _numbers(actions[count]['click'], 3)
        count += 1
    values = actions[count:]
    if 'selection_choices' in op and (not isinstance(op['selection_choices'],list)
            or len(op['selection_choices'])!=count
            or any(type(i)is not int or not 0<=i<32 for i in op['selection_choices'])):
        raise ValueError('invalid per-click overlap choices')
    if not values and count==2 and op.get('alternate_family',op['family'])!=op['family']:return
    if values in (['Automatic'], ['Cancel']): return
    dimension = 2 if op['family'] in CURVES else 4
    if len(values) not in ((2,4,6) if op.get('alternate_family',op['family']) != op['family'] else (dimension, dimension * count)):
        raise ValueError('wrong domain input count')
    for value in values:
        if value == 'Enter': continue
        if type(value) not in (int, float) or math.isnan(value) or math.isinf(value):
            raise ValueError('invalid domain getter input')
    for a, b in zip(values[::2], values[1::2]):
        if a != 'Enter' and b != 'Enter' and not a < b:
            raise ValueError('domain end must exceed its start')


def run(op, host, iterations, tolerance):
    validate(op)
    if iterations != 1: raise ValueError('block reparameterization requires one iteration')
    R, S = host['Rhino'], host['System']
    G = R.Geometry
    from join_probe import observe_command
    from conic_getter_input import drive
    doc = R.RhinoDoc.ActiveDoc
    if R.Commands.Command.InCommand() or list(doc.Objects):
        raise ValueError('block reparameterization needs an owned empty idle document')
    owned, attrs, definitions, layers, groups, roots = [], [], [], [], [], []
    names = {}
    prefix = 'VibocerosReparameterize_' + str(S.Guid.NewGuid()) + '_'
    original_layer = doc.Layers.CurrentLayerIndex
    original_tolerance = doc.ModelAbsoluteTolerance
    doc.ModelAbsoluteTolerance = tolerance['absolute']
    def keep(g):
        if g is None or not g.IsValid: raise ValueError('invalid block source geometry')
        owned.append(g);return g
    def matrix(rows):
        m = G.Transform.Identity
        for i in range(4):
            for j in range(4): m[i,j] = rows[i][j]
        return m
    def transform_record(m):return [[float(m[i,j]) for j in range(4)] for i in range(4)]
    def strings(value):
        data = value.GetUserStrings()
        return dict((str(k), str(data[k])) for k in data.AllKeys) if data else {}
    def attribute(name, layer, code, parent=True):
        a = R.DocObjects.ObjectAttributes();attrs.append(a)
        a.Name = name;a.LayerIndex = layer;a.SetUserString('Code', code)
        a.ObjectColor = S.Drawing.Color.FromArgb(75, 125, 175)
        a.ColorSource = R.DocObjects.ObjectColorSource.ColorFromParent if parent else R.DocObjects.ObjectColorSource.ColorFromObject
        return a
    def attributes(a):
        return dict(name=str(a.Name) if a.Name else None, layer=names.get(('layer',a.LayerIndex), 'original'),
            color_source=str(a.ColorSource), color=[int(a.ObjectColor.R),int(a.ObjectColor.G),int(a.ObjectColor.B)],
            user_text=strings(a),groups=[names.get(('group',int(i)),str(i)) for i in (a.GetGroupList() or [])],
            visible=bool(a.Visible),locked=str(a.Mode)=='Locked')
    def geometry(g):
        if isinstance(g,G.InstanceReferenceGeometry):
            return dict(type='instance',definition=names[g.ParentIdefId],transform=transform_record(g.Xform))
        if isinstance(g,G.Brep):return dict(type='brep',value=host['_interchange_brep_record'](g))
        return dict(type='curve',value=host['_interchange_curve_record'](g))
    def object_record(obj):
        return dict(id=str(obj.Id),attributes=attributes(obj.Attributes),geometry=geometry(obj.Geometry),
            geometry_text=strings(obj.Geometry),selected=bool(obj.IsSelected(False)))
    def snapshot():
        return dict(objects=[object_record(o) for o in doc.Objects],definitions=[dict(name=names[doc.InstanceDefinitions[i].Id],
            id=str(doc.InstanceDefinitions[i].Id),members=[object_record(o) for o in doc.InstanceDefinitions[i].GetObjects()]) for i in definitions])
    try:
        for label,color in [('Source',(210,40,40)),('Instance',(40,180,40)),('Current',(40,40,210))]:
            layer=R.DocObjects.Layer();layer.Name=prefix+label;layer.Color=S.Drawing.Color.FromArgb(*color)
            try:index=doc.Layers.Add(layer)
            finally:layer.Dispose()
            if index<0:raise ValueError('block source layer creation failed')
            layers.append(index);names[('layer',index)]=label
        doc.Layers.SetCurrentLayerIndex(layers[2],True)
        for label in ['PrototypeGroup','RootGroup']:
            index=doc.Groups.Add(prefix+label)
            if index<0:raise ValueError('block source group creation failed')
            groups.append(index);names[('group',index)]=label
        def make_source(family):
            if family=='line':return keep(G.LineCurve(G.Point3d(0.,0.,0.),G.Point3d(3.,4.,0.)))
            elif family=='circle':return keep(G.ArcCurve(G.Circle(G.Plane.WorldXY,2.)))
            elif family=='nurbs':return keep(G.NurbsCurve.Create(False,3,S.Array[G.Point3d]([G.Point3d(0.,0.,0.),G.Point3d(1.,3.,0.),G.Point3d(3.,-1.,0.),G.Point3d(5.,2.,0.)])))
            elif family=='annulus':
                curves=[keep(G.Circle(G.Plane.WorldXY,r).ToNurbsCurve())for r in (5.,2.)]
                parts=list(G.Brep.CreatePlanarBreps(curves,tolerance['absolute'])or [])
                if len(parts)!=1:raise ValueError('annulus construction failed')
                return keep(parts[0])
            elif family=='cylinder':return keep(keep(G.Cylinder(G.Circle(G.Plane.WorldXY,2.),3.).ToNurbsSurface()).ToBrep())
            elif family=='sphere':return keep(keep(G.Sphere(G.Point3d.Origin,2.).ToNurbsSurface()).ToBrep())
            else:return keep(G.BoundingBox(G.Point3d.Origin,G.Point3d(4.,6.,2.)).ToBrep())
        source=make_source(op['family'])
        source.SetUserString('GeometryCode','prototype')
        alternate=make_source(op['alternate_family']) if 'alternate_family' in op else keep(source.Duplicate())
        if 'alternate_domain' in op:alternate.Domain=G.Interval(*op['alternate_domain'])
        if not alternate.Transform(G.Transform.Translation(30.,0.,0.)):raise ValueError('alternate member placement failed')
        alternate.SetUserString('GeometryCode','alternate')
        a=attribute('Target',layers[0],'prototype');a.AddToGroup(groups[0])
        b=attribute('Alternate',layers[0],'alternate',False)
        definition=doc.InstanceDefinitions.Add(prefix+'Leaf','',G.Point3d.Origin,S.Array[G.GeometryBase]([source,alternate]),S.Array[R.DocObjects.ObjectAttributes]([a,b]))
        if definition<0:raise ValueError('leaf definition creation failed')
        definitions.append(definition);leaf=definition;names[doc.InstanceDefinitions[definition].Id]='Leaf'
        placements=list(map(matrix,op['placements']))
        for level in reversed(range(len(placements)-1)):
            identifier=doc.InstanceDefinitions[definition].Id
            refs=[keep(G.InstanceReferenceGeometry(identifier,placements[level+1])),
                keep(G.InstanceReferenceGeometry(identifier,G.Transform.Translation(40.,0.,0.)*placements[level+1]))]
            aa=[attribute('Node%d_%d'%(level,i),layers[1],'node',True)for i in range(2)]
            definition=doc.InstanceDefinitions.Add(prefix+'Node'+str(level),'',G.Point3d.Origin,S.Array[G.GeometryBase](refs),S.Array[R.DocObjects.ObjectAttributes](aa))
            if definition<0:raise ValueError('nested definition creation failed')
            definitions.append(definition);names[doc.InstanceDefinitions[definition].Id]='Node'+str(level)
        for index,placement in enumerate([placements[0],G.Transform.Translation(2000.,0.,0.)*placements[0]]):
            a=attribute('Root'+str(index),layers[1],'root',False);a.AddToGroup(groups[1])
            a.ObjectColor=S.Drawing.Color.FromArgb(20,180,60) if index==0 else S.Drawing.Color.FromArgb(220,160,20)
            key=doc.Objects.AddInstanceObject(definition,placement,a)
            if key==S.Guid.Empty:raise ValueError('root insertion failed')
            roots.append(key)
        composed=placements[0]
        for level in range(len(placements)-1):
            composed=composed*G.Transform.Translation(40.*op['member_path'][level],0.,0.)*placements[level+1]
        prototype=doc.InstanceDefinitions[leaf].Object(op['member_path'][-1])
        original=prototype.Geometry
        placed=keep(original.ToNurbsCurve() if isinstance(original,G.ArcCurve) and composed.SimilarityType==G.TransformSimilarityType.NotSimilarity else original.Duplicate())
        if not placed.Transform(composed):raise ValueError('placed source witness transformation failed')
        witness=dict(attributes=attributes(prototype.Attributes),geometry=geometry(placed),geometry_text=strings(original),transform=transform_record(composed),prototype_id=str(prototype.Id))
        path=op['artifact_path']
        if path.startswith('/'):path='Z:'+path.replace('/','\\')
        options=R.FileIO.FileWriteOptions();options.FileVersion=8;options.SuppressDialogBoxes=True;options.SuppressAllInput=True;options.WriteSelectedObjectsOnly=False;options.WriteUserData=True;options.UpdateDocumentPath=False
        try:
            if not doc.Write3dmFile(path,options):raise ValueError('block source artifact write failed')
        finally:options.Dispose()
        R.RhinoApp.RunScript('_SetView _World _'+op['view'],False)
        viewport=doc.Views.ActiveView.ActiveViewport
        display=R.Display.DisplayModeDescription.FindByName(op.get('display','Shaded'))
        if display is None:raise ValueError('block pick display mode unavailable')
        viewport.DisplayMode=display
        bounds=placed.GetBoundingBox(True);bounds.Inflate(max(bounds.Diagonal.Length*.2,1.))
        for action in op['actions']:
            if isinstance(action,dict):bounds.Union(G.Point3d(*action['click']))
        bounds.Inflate(1.)
        if not viewport.ZoomBoundingBox(bounds):raise ValueError('block pick viewport framing failed')
        doc.Views.Redraw();doc.Objects.UnselectAll();doc.ClearUndoRecords(True)
        before=snapshot();trace=[]
        marker='Viboceros block Reparameterize '+str(S.Guid.NewGuid());R.RhinoApp.WriteLine(marker)
        def action():
            success,input_record=drive(op,host,[],command='Reparameterize');trace.append(input_record)
            host['_record_progress'](op['id']+': getter returned')
            return success
        success,after,events=observe_command(R.Commands.Command,'Reparameterize',action,snapshot,lambda:[],True)
        host['_record_progress'](op['id']+': command observed')
        if R.Commands.Command.InCommand():raise ValueError('block command remains active')
        history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[-1]
        undo=redo=None
        if any(o['id']not in set(str(k)for k in roots)for o in after['objects']):
            R.RhinoApp.RunScript('_Undo',False);undo=snapshot()
            host['_record_progress'](op['id']+': undo observed')
            R.RhinoApp.RunScript('_Redo',False);redo=snapshot()
            host['_record_progress'](op['id']+': redo observed')
        return dict(success=success,before=before,after=after,undo=undo,redo=redo,history=history,
            events=events,input=trace,placed_source=witness,root_id=str(roots[0]),member_path=op['member_path']),0
    finally:
        if R.Commands.Command.InCommand():R.RhinoApp.RunScript('!',False)
        for obj in list(doc.Objects):doc.Objects.Delete(obj.Id,True)
        for index in reversed(definitions):
            if not doc.InstanceDefinitions[index].IsDeleted:doc.InstanceDefinitions.Delete(index,True,True)
        for index in groups:
            if not doc.Groups.IsDeleted(index):doc.Groups.Delete(index)
        doc.Layers.SetCurrentLayerIndex(original_layer,True)
        for index in reversed(layers):doc.Layers.Delete(index,True)
        doc.ModelAbsoluteTolerance=original_tolerance
        for a in reversed(attrs):a.Dispose()
        for g in reversed(owned):g.Dispose()
