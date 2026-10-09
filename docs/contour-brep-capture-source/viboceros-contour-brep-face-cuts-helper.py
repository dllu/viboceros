# -*- coding: utf-8 -*-
"""Owned native Contour command geometry, property and group observations."""
import math

def validate(operation):
    required=set(('op','id','sources','start','end','spacing'))
    allowed=required|set(('origin','x_axis','y_axis','range','properties','group'))
    if not required.issubset(operation) or set(operation)-allowed:raise ValueError('invalid Contour fields')
    sources=operation['sources']
    if not isinstance(sources,list) or not 1<=len(sources)<=64:raise ValueError('invalid Contour sources')
    if any(not isinstance(source,dict) for source in sources) or not any(source.get('type') not in ('point','point_cloud') for source in sources):raise ValueError('Contour requires an eligible curve, surface, Brep or mesh source')
    for name,default in [('start',None),('end',None),('origin',[0,0,0]),('x_axis',[1,0,0]),('y_axis',[0,1,0])]:
        value=operation.get(name,default)
        if not isinstance(value,list) or len(value)!=3 or any(type(v) not in (int,float) or math.isnan(v) or math.isinf(v) for v in value):raise ValueError('invalid finite Contour coordinates')
    if type(operation['spacing']) not in (int,float) or not 0 < operation['spacing'] < float('inf'):raise ValueError('invalid positive Contour spacing')
    if type(operation.get('range',False))is not bool or type(operation.get('group',False))is not bool or operation.get('properties','current')not in ('current','input'):raise ValueError('invalid Contour options')

def run(operation,tolerance,host):
    validate(operation)
    import Rhino,System
    document=Rhino.RhinoDoc.ActiveDoc
    prefix='VibocerosOracleContour_'+str(System.Guid.NewGuid())+'_'
    ids=[];layers=[];outputs=[];native_faces=[];native_face_cuts=[]
    def xyz(value):return [float(value.X),float(value.Y),float(value.Z)]
    def record(obj):
        geometry=obj.Geometry;attributes=obj.Attributes
        if isinstance(geometry,Rhino.Geometry.Point):kind='point';points=[xyz(geometry.Location)];domain=None;closed=False
        elif isinstance(geometry,Rhino.Geometry.Curve):
            kind='curve';domain=[float(geometry.Domain.T0),float(geometry.Domain.T1)];closed=bool(geometry.IsClosed)
            points=[xyz(geometry.PointAt(geometry.Domain.ParameterAt(i/16.))) for i in range(17)]
        else:raise ValueError('unsupported native Contour output')
        if not geometry.IsValid:raise ValueError('invalid native Contour output')
        native_type=str(geometry.GetType().Name)
        native_controls=None
        if isinstance(geometry,Rhino.Geometry.Curve):
            ok,polyline=geometry.TryGetPolyline()
            if ok:native_controls=[xyz(point)for point in polyline]
        color=attributes.ObjectColor
        return dict(native_type=native_type,native_controls=native_controls,kind=kind,points=points,domain=domain,closed=closed,name=str(attributes.Name)if attributes.Name else None,layer='Current' if attributes.LayerIndex==layers[-1] else 'Source'+str(layers.index(attributes.LayerIndex)),color=[int(color.R),int(color.G),int(color.B)],color_source=str(attributes.ColorSource),groups=[int(i)-group_start for i in (attributes.GetGroupList()or[])])
    old_layer=document.Layers.CurrentLayerIndex
    group_start=document.Groups.Count
    try:
        for i,source in enumerate(operation['sources']):
            layer=Rhino.DocObjects.Layer();layer.Name=prefix+'Source'+str(i);index=document.Layers.Add(layer);layer.Dispose();layers.append(index)
            geometry=host['_object_source'](source,tolerance);attributes=Rhino.DocObjects.ObjectAttributes();attributes.LayerIndex=index;attributes.Name='source-'+str(i);attributes.ObjectColor=System.Drawing.Color.FromArgb(21+i,43+i,65+i);attributes.ColorSource=Rhino.DocObjects.ObjectColorSource.ColorFromObject
            if isinstance(geometry,Rhino.Geometry.Brep):
                native_faces.append([dict(index=int(face.FaceIndex),center=xyz(face.PointAt(face.Domain(0).Mid,face.Domain(1).Mid)),normal=xyz(face.NormalAt(face.Domain(0).Mid,face.Domain(1).Mid)),domains=[[float(face.Domain(i).T0),float(face.Domain(i).T1)]for i in range(2)],reversed=bool(face.OrientationIsReversed)) for face in geometry.Faces])
            if isinstance(geometry,Rhino.Geometry.Brep) and geometry.Faces.Count>1:
                normal=host['_point'](operation['end'])-host['_point'](operation['start']);normal.Unitize()
                origin=host['_point'](operation['start'])-normal*2.0
                plane=Rhino.Geometry.Plane(origin,normal);cuts=[]
                for face in geometry.Faces:
                    single=face.DuplicateFace(False)
                    curves=Rhino.Geometry.Brep.CreateContourCurves(single,plane)
                    cuts.append(dict(face=int(face.FaceIndex),curves=[dict(start=xyz(curve.PointAtStart),end=xyz(curve.PointAtEnd),domain=[float(curve.Domain.T0),float(curve.Domain.T1)],closed=bool(curve.IsClosed)) for curve in curves]))
                    for curve in curves:curve.Dispose()
                    single.Dispose()
                native_face_cuts.append(cuts)
            identifier=document.Objects.Add(geometry,attributes);geometry.Dispose();attributes.Dispose()
            if identifier==System.Guid.Empty:raise ValueError('native Contour source admission failed')
            ids.append(identifier)
        layer=Rhino.DocObjects.Layer();layer.Name=prefix+'Current';index=document.Layers.Add(layer);layer.Dispose();layers.append(index);document.Layers.SetCurrentLayerIndex(index,True)
        frame=Rhino.Geometry.Plane(host['_point'](operation.get('origin',[0,0,0])),host['_vector'](operation.get('x_axis',[1,0,0])),host['_vector'](operation.get('y_axis',[0,1,0])))
        document.Views.ActiveView.ActiveViewport.SetConstructionPlane(frame)
        document.Objects.UnselectAll()
        settings=Rhino.DocObjects.ObjectEnumeratorSettings();before=set(o.Id for o in document.Objects.GetObjectList(settings))
        options='_AssignProperties='+('_ByCurrentLayer'if operation.get('properties','current')=='current'else '_ByInputObject')+' _Output=_CurvesOnly _GroupObjectsByContourPlane='+('_Yes'if operation.get('group',False)else '_No')
        script='_Contour '+options+' '+' '.join('_SelID '+str(key) for key in ids)+' _Enter '+('_Range ' if operation.get('range',False) else '')+'w'+','.join('%.17g'%v for v in operation['start'])+' w'+','.join('%.17g'%v for v in operation['end'])+' '+'%.17g'%operation['spacing']
        succeeded=Rhino.RhinoApp.RunScript(script,False)
        outputs=[o.Id for o in document.Objects.GetObjectList(settings) if o.Id not in before and not o.IsDeleted and not o.IsInstanceDefinitionGeometry]
        records=[record(document.Objects.FindId(key)) for key in outputs]

        return dict(native_face_cuts=native_face_cuts,native_faces=native_faces,succeeded=bool(succeeded),outputs=records,input_count=sum(document.Objects.FindId(key)is not None and not document.Objects.FindId(key).IsDeleted for key in ids)),0
    finally:
        document.Layers.SetCurrentLayerIndex(old_layer,True)
        for key in outputs+ids:document.Objects.Delete(key,True)
        for index in range(group_start,document.Groups.Count):document.Groups.Delete(index)
        for index in reversed(layers):document.Layers.Delete(index,True)
