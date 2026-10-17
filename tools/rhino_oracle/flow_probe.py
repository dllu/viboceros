"""Authored curve-to-curve Flow SDK witnesses and actual command diagnostics."""
import math,re
if globals().get('__package__'):
    from .geometry_validity_probe import validate as validate_source
else:
    from geometry_validity_probe import validate as validate_source

def validate(op):
    if not isinstance(op,dict)or op.get('op')!='flow' or not {'op','id','base','target','points'}<=set(op) or set(op)-{'op','id','base','target','points','options','command','group','diagnostics','diagnostic_length_samples','morph_tolerance','quick_preview','roadlike_syntax'}:raise ValueError('invalid Flow recipe')
    if not isinstance(op['id'],str)or re.match(r'\A[A-Za-z0-9_-]{1,100}\Z',op['id'])is None:raise ValueError('invalid Flow id')
    for key in ('base','target'):
        if not isinstance(op[key],dict)or op[key].get('type')not in ('line','arc','circle','ellipse','nurbs','polyline','polycurve'):raise ValueError('Flow requires curve guides')
        validate_source(dict(op='geometry_validity',id=op['id'],source=op[key]))
    if not isinstance(op['points'],list)or not 4<=len(op['points'])<=2048:raise ValueError('invalid Flow witness count')
    for p in op['points']:
        if not isinstance(p,list)or len(p)!=3 or any(type(x)not in(int,float)or math.isnan(x)or math.isinf(x)for x in p):raise ValueError('invalid Flow witness')
    o=op.get('options',{})
    if not isinstance(o,dict)or set(o)-{'reverse_base','reverse_target','stretch','copy','rigid','rigid_groups','preserve_structure','roadlike','stairlike'}or any(type(v)is not bool for v in o.values()):raise ValueError('invalid Flow options')
    if o.get('stairlike'):raise ValueError('Stairlike capture is not qualified')
    if op.get('roadlike_syntax',0)not in(0,1,2,3):raise ValueError('invalid roadlike capture sequence')
    if 'morph_tolerance'in op and(type(op['morph_tolerance'])not in(int,float)or (math.isnan(op['morph_tolerance'])or math.isinf(op['morph_tolerance']))or op['morph_tolerance']<=0):raise ValueError('invalid Flow morph tolerance')
    for k in ('command','group','diagnostics','quick_preview'):
        if type(op.get(k,False))is not bool:raise ValueError('invalid Flow capture flag')
    if 'diagnostic_length_samples'in op and(type(op['diagnostic_length_samples'])is not int or not 2<=op['diagnostic_length_samples']<=129 or not op.get('diagnostics')):raise ValueError('invalid Flow length diagnostic count')

def run(op,host,iterations,tolerance):
    validate(op)
    if iterations!=1:raise ValueError('Flow capture requires one iteration')
    R=host['Rhino'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc;o=op.get('options',{})
    base=host['_object_source'](op['base'],tolerance);target=host['_object_source'](op['target'],tolerance)
    morph=G.Morphs.FlowSpaceMorph(base,target,o.get('reverse_base',False),o.get('reverse_target',False),not o.get('stretch',True))
    if 'morph_tolerance'in op:morph.Tolerance=float(op['morph_tolerance'])
    if 'quick_preview'in op:morph.QuickPreview=op['quick_preview']
    before=set(obj.Id for obj in doc.Objects);groups=set(g.Index for g in doc.Groups if not g.IsDeleted)
    try:
        points=[G.Point3d(*p)for p in op['points']]
        result=dict(sdk_points=[host['_xyz'](morph.MorphPoint(p))for p in points],base_length=base.GetLength(),target_length=target.GetLength(),command=None)
        if op.get('diagnostics'):
            def plane(p):return dict(origin=host['_xyz'](p.Origin),x=host['_xyz'](p.XAxis),y=host['_xyz'](p.YAxis),z=host['_xyz'](p.ZAxis))
            def frames(c):
                parameters=[c.Domain.ParameterAt(f)for f in(0,.25,.5,.75,1)]
                out=[]
                for t in parameters:
                    row=dict(t=t,point=host['_xyz'](c.PointAt(t)),tangent=host['_xyz'](c.TangentAt(t)))
                    for name,method in [('frame',c.FrameAt),('perp',c.PerpendicularFrameAt)]:
                        ok,p=method(t)
                        if ok:row[name]=plane(p)
                    out.append(row)
                ok,p=c.TryGetPlane();batch=c.GetPerpendicularFrames(host['System'].Array[host['System'].Double](parameters))
                result=dict(rows=out,plane=plane(p)if ok else None,batch=[plane(p)for p in batch]if batch is not None else None,linear=[dict(tolerance=t,value=bool(c.IsLinear(t)))for t in(1e-6,1e-4,.001,.01,.1,1.)])
                if 'diagnostic_length_samples'in op:
                    rows=[];parameters=[]
                    for i in range(op['diagnostic_length_samples']):
                        fraction=float(i)/(op['diagnostic_length_samples']-1)
                        ok,t=c.NormalizedLengthParameter(fraction,1e-12)
                        if not ok:raise ValueError('public normalized length lookup failed')
                        row=dict(fraction=fraction,t=t,point=host['_xyz'](c.PointAt(t)),tangent=host['_xyz'](c.TangentAt(t)))
                        lookups=[]
                        for error in (1e-2,1e-4,1e-6,1e-8):
                            success,parameter=c.NormalizedLengthParameter(fraction,error)
                            if not success:raise ValueError('public fractional length lookup failed')
                            lookups.append(dict(fractional_tolerance=error,t=parameter))
                        row['length_lookups']=lookups
                        for name,method in [('frame',c.FrameAt),('perp',c.PerpendicularFrameAt)]:
                            success,frame=method(t)
                            if not success:raise ValueError('public length frame query failed')
                            row[name]=plane(frame)
                        rows.append(row);parameters.append(t)
                    batch=c.GetPerpendicularFrames(host['System'].Array[host['System'].Double](parameters))
                    if batch is None or len(batch)!=len(rows):raise ValueError('public length batch frames failed')
                    result['length_rows']=rows;result['length_batch']=[plane(p)for p in batch]
                    classifications=[]
                    for error in (1e-6,.001,.01,.05,.1,.2,1.):
                        success,frame=c.TryGetPlane(error)
                        classifications.append(dict(tolerance=error,linear=bool(c.IsLinear(error)),plane=plane(frame)if success else None))
                    result['tolerance_classification']=classifications
                return result
            result['diagnostics']=dict(base=frames(base),target=frames(target))
        if not op.get('command'):return result,0
        # Reverse copies of the authored guide definitions; the SDK constructor
        # received the same explicit direction choices on the original curves.
        if o.get('reverse_base'):base.Reverse()
        if o.get('reverse_target'):target.Reverse()
        a=doc.Objects.AddCurve(base);b=doc.Objects.AddCurve(target);cloud=G.PointCloud(points)
        try:source=doc.Objects.AddPointCloud(cloud)
        finally:cloud.Dispose()
        if op.get('group'):doc.Groups.Add('Owned Flow source',[source])
        definition=lambda key:host['_nurbs_curve_definition'](doc.Objects.FindId(key).Geometry)
        guides=[definition(a),definition(b)];doc.Objects.UnselectAll();doc.Objects.Select(source,True)
        flags=' '.join('_'+key+'=_'+('Yes'if o.get(field,default)else'No')for key,field,default in [('Copy','copy',False),('Rigid','rigid',False),('Stretch','stretch',True),('PreserveStructure','preserve_structure',False)])
        flags+=' _Roadlike=_'+('Yes'if o.get('roadlike')else'No')
        if o.get('rigid'):flags+=' _RigidGroups=_'+('Yes'if o.get('rigid_groups',True)else'No')
        marker='Owned Flow '+str(host['System'].Guid.NewGuid());R.RhinoApp.WriteLine(marker)
        point=lambda p:'w'+','.join(repr(float(v))for v in p)
        def axis(c):
            if not o.get('roadlike'):return ''
            style=op.get('roadlike_syntax',0);p=host['_xyz'](c.PointAtStart)
            return [' _Automatic',' _Automatic _Enter',' '+point(p)+' _Automatic',' '+point(p)+' '+point([p[0],p[1],p[2]+1])][style]
        macro='_Flow '+flags+" '_-SelID "+str(a)+" '_-SelID "+str(b)+axis(base)+axis(target)+' _Cancel'
        returned=R.RhinoApp.RunScript(macro,True);outputs=[]
        for obj in doc.Objects:
            if obj.Id in before or obj.Id in(a,b)or not isinstance(obj.Geometry,G.PointCloud):continue
            outputs.append(dict(original=obj.Id==source,selected=bool(obj.IsSelected(False)),group_count=obj.GroupCount,points=[host['_xyz'](p.Location)for p in obj.Geometry]))
        result['command']=dict(returned=bool(returned),macro=macro,history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[-1].strip(),outputs=outputs,guides_unchanged=guides==[definition(a),definition(b)],guides_selected=[bool(doc.Objects.FindId(key).IsSelected(False))for key in(a,b)],group_count=sum(not g.IsDeleted and g.Index not in groups for g in doc.Groups))
        return result,0
    finally:
        for obj in list(doc.Objects):
            if obj.Id not in before:doc.Objects.Delete(obj.Id,True)
        for g in list(doc.Groups):
            if not g.IsDeleted and g.Index not in groups:doc.Groups.Delete(g.Index)
        if hasattr(morph,'Dispose'):morph.Dispose()
        base.Dispose();target.Dispose()
