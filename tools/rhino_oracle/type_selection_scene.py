# coding: utf-8
"""Public SDK scene with nested blocks, reflected placements and real extrusions."""


def build(R,S,doc,op):
    G=R.Geometry
    keys={}
    layer_index=doc.Layers.FindByFullPath('Selection scope',-1)
    if layer_index<0:
        layer=R.DocObjects.Layer();layer.Name='Selection scope'
        try:layer_index=doc.Layers.Add(layer)
        finally:layer.Dispose()
    if layer_index<0:raise ValueError('selection layer admission failed')
    def attrs(name):
        a=R.DocObjects.ObjectAttributes();a.Name=name
        a.SetUserString('Owned selection','unchanged')
        a.LayerIndex=layer_index if name in ('mirror','extrusion_closed','unicode') else doc.Layers.CurrentLayerIndex
        return a
    def definition(name,content):
        a=[R.DocObjects.ObjectAttributes() for unused in content]
        try:
            index=doc.InstanceDefinitions.Add(name,'Owned selection reference',G.Point3d.Origin,content,a)
            if index<0:raise ValueError('block definition admission failed: '+name)
            return index
        finally:
            for item in a:item.Dispose()
            for item in content:item.Dispose()
    leaf=definition('Bolt A',[G.Point(G.Point3d(1,2,3))])
    other=definition('Other',[G.Point(G.Point3d(2,1,0))])
    unicode=definition(u'部品 A',[G.Point(G.Point3d(3,1,2))])
    reflected=G.Transform.Identity;reflected.M00=-1.
    nested=definition('Nested',[G.InstanceReferenceGeometry(doc.InstanceDefinitions[leaf].Id,reflected)])
    def placement(sx,sy,sz,x,shear=0.):
        t=G.Transform.Identity;t.M00=sx;t.M11=sy;t.M22=sz;t.M01=shear;t.M03=x
        return t
    for name,index,t in [
        ('plain',leaf,placement(1,1,1,0)),('mirror',leaf,placement(-1,1,1,10)),
        ('double',leaf,placement(-1,-1,1,20)),('triple',leaf,placement(-1,-1,-1,30)),
        ('shear',leaf,placement(-2,3,4,40,2)),('nested_positive',nested,placement(1,1,1,50)),
        ('nested_negative',nested,placement(-1,1,1,60)),('other',other,placement(1,1,1,70)),
        ('unicode',unicode,placement(1,1,1,80))]:
        a=attrs(name)
        try:keys[name]=doc.Objects.AddInstanceObject(index,t,a)
        finally:a.Dispose()
    for name,cap in [('extrusion_open',False),('extrusion_closed',True),('explicit_brep',True)]:
        curve=G.Rectangle3d(G.Plane.WorldXY,4.,3.).ToNurbsCurve()
        try:g=G.Extrusion.Create(curve,2.,cap)
        finally:curve.Dispose()
        if name=='explicit_brep':
            b=g.ToBrep();g.Dispose();g=b
        a=attrs(name)
        try:keys[name]=doc.Objects.Add(g,a)
        finally:g.Dispose();a.Dispose()
    a=attrs('point')
    try:keys['point']=doc.Objects.AddPoint(G.Point3d(5,6,7),a)
    finally:a.Dispose()
    if any(key==S.Guid.Empty for key in keys.values()):raise ValueError('source admission failed')
    variant=op['variant']
    if variant=='grouped':doc.Groups.Add('Owned mixed group',[keys[n] for n in ('mirror','point','extrusion_closed')])
    if variant=='restricted':
        for n in ('mirror','extrusion_closed'):doc.Objects.Hide(keys[n],True)
        for n in ('triple','extrusion_open'):doc.Objects.Lock(keys[n],True)
    if variant in ('hidden_layer','locked_layer'):
        layer=doc.Layers[layer_index]
        if variant=='hidden_layer':layer.IsVisible=False
        else:layer.IsLocked=True
        if not doc.Layers.Modify(layer,layer_index,True):raise ValueError('layer restriction failed')
    if variant=='no_matches':
        for n,key in keys.items():
            if n!='point':doc.Objects.Lock(key,True)
    return keys,layer_index
