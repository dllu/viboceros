"""Owned Divide command geometry, source metadata and output groups."""
import math

def validate(operation):
    required={'op','id','sources','mode','value'}
    if not required.issubset(operation) or set(operation)-required-{'mark_ends','split','delete_remainder','group'}:
        raise ValueError('invalid Divide fields')
    if not isinstance(operation['sources'],list) or not 1<=len(operation['sources'])<=64 or any(not isinstance(s,dict) or s.get('type') not in ('line','circle','arc','ellipse','polyline','nurbs','polycurve') for s in operation['sources']):
        raise ValueError('invalid Divide sources')
    if operation['mode'] not in ('count','length','chord') or type(operation['value']) not in (int,float) or (math.isnan(operation['value']) or math.isinf(operation['value'])) or operation['value']<=0:
        raise ValueError('invalid Divide mode/value')
    if operation['mode']=='count' and (operation['value']!=int(operation['value']) or operation['value']>1000000):
        raise ValueError('invalid Divide segment count')
    for key in ('mark_ends','split','delete_remainder','group'):
        if type(operation.get(key,False))is not bool:raise ValueError('invalid Divide option')

def run(operation, tolerance, host):
    validate(operation)
    import Rhino, System
    doc=Rhino.RhinoDoc.ActiveDoc
    sources=[];outputs=[];layers=[]
    original_layer=doc.Layers.CurrentLayerIndex
    settings=Rhino.DocObjects.ObjectEnumeratorSettings()
    baseline=set(o.Id for o in doc.Objects.GetObjectList(settings))
    try:
        for i,source in enumerate(operation['sources']):
            layer=Rhino.DocObjects.Layer();layer.Name='DivideSource_'+str(System.Guid.NewGuid());index=doc.Layers.Add(layer);layer.Dispose();layers.append(index)
            geometry=host['_object_source'](source,tolerance)
            attributes=Rhino.DocObjects.ObjectAttributes();attributes.LayerIndex=index;attributes.Name='Source'+str(i);attributes.ObjectColor=System.Drawing.Color.FromArgb(20+i,40,60);attributes.ColorSource=Rhino.DocObjects.ObjectColorSource.ColorFromObject
            try:sources.append(doc.Objects.Add(geometry,attributes))
            finally:geometry.Dispose();attributes.Dispose()
        doc.Layers.SetCurrentLayerIndex(original_layer,True)
        doc.Objects.UnselectAll()
        for key in sources:doc.Objects.Select(key)
        mode=operation['mode']
        split=operation.get('split',False)
        delete=operation.get('delete_remainder',False)
        mark=operation.get('mark_ends',False)
        options='_Split='+('_Yes'if split else '_No')+' _GroupOutput='+('_Yes'if operation.get('group',False)else'_No')
        if not split and not all(doc.Objects.FindId(k).Geometry.IsClosed for k in sources):options+=' _MarkEnds='+('_Yes'if mark else'_No')

        mode_option={'count':'','length':'_Length ','chord':'_EqualChordLength '}[mode]
        if split and mode!='count':mode_option += '_DeleteRemainder='+('_Yes'if delete else'_No')+' '
        macro='_Divide '+options+' '+mode_option+'%.17g'%operation['value']+' _Enter'
        if any(doc.Objects.FindId(k).Geometry.IsClosed for k in sources):macro=macro.replace('_Divide ','_Divide _Enter ',1)
        ok=Rhino.RhinoApp.RunScript(macro,True)
        objects=sorted((o for o in doc.Objects.GetObjectList(settings)if o.Id not in baseline and o.Id not in sources),key=lambda o:int(o.RuntimeSerialNumber))
        outputs=[o.Id for o in objects]
        records=[];group_map={}
        for obj in objects:
            g=obj.Geometry;a=obj.Attributes;groups=[]
            for n in a.GetGroupList()or[]:
                if n not in group_map:group_map[n]=len(group_map)
                groups.append(group_map[n])
            if isinstance(g,Rhino.Geometry.Point):kind='point';points=[[float(g.Location.X),float(g.Location.Y),float(g.Location.Z)]];domain=None;closed=False
            else:
                kind='curve';domain=[float(g.Domain.T0),float(g.Domain.T1)];closed=bool(g.IsClosed);points=[]
                for i in range(17):
                    t=g.Domain.T0 if i==0 else g.Domain.T1 if i==16 else g.Domain.T0+(g.Domain.T1-g.Domain.T0)*i/16.
                    p=g.PointAt(t);points.append([float(p.X),float(p.Y),float(p.Z)])
            records.append(dict(kind=kind,points=points,domain=domain,closed=closed,name=a.Name or '',layer='current'if a.LayerIndex==original_layer else'source',color_source=str(a.ColorSource),groups=groups))
        result=dict(succeeded=bool(ok),outputs=records,input_count=sum(doc.Objects.FindId(k)is not None and not doc.Objects.FindId(k).IsDeleted for k in sources),group_count=len(group_map))
        return result,0
    finally:
        for key in outputs+sources:doc.Objects.Delete(key,True)
        for index in reversed(layers):doc.Layers.Delete(index,True)
