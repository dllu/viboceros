"""Owned OrientOnSrf control/edit grips with complete owner snapshots."""
import math
import re
if globals().get('__package__'):
    from .geometry_validity_probe import validate as validate_geometry
    from .orient_surface_probe import validate as validate_placement
else:
    from geometry_validity_probe import validate as validate_geometry
    from orient_surface_probe import validate as validate_placement

def validate(op):
    allowed={'op','id','surface','source','selected','base','reference','uv','options','edit','parent','peer','group','postselected','batch'}
    if not isinstance(op,dict)or set(op)-allowed or not {'op','id','surface','source','selected','base','reference','uv','options'}<=set(op)or op['op']!='orient_surface_grips':raise ValueError('invalid surface grip recipe')
    placement={k:v for k,v in op.items()if k in {'id','surface','base','reference','uv','options','group','postselected','batch'}}
    placement['op']='orient_surface';validate_placement(placement)
    validate_geometry(dict(op='geometry_validity',id=op['id'],source=op['source']))
    if op['source'].get('type')not in('nurbs','surface','mesh'):raise ValueError('unsupported grip recipe family')
    if not isinstance(op['selected'],list)or not 1<=len(op['selected'])<=64 or any(type(i)is not int or not 0<=i<64 for i in op['selected'])or len(set(op['selected']))!=len(op['selected']):raise ValueError('invalid selected grips')
    for k in('edit','parent','peer','group','postselected'):
        if type(op.get(k,False))is not bool:raise ValueError('invalid grip toggle')
    if op['options'].get('on_surface'):raise ValueError('OnSurface grip capture is not qualified')
    if op.get('edit')and op['source']['type']!='nurbs':raise ValueError('edit points require a curve')
    if op.get('postselected')and op.get('parent'):raise ValueError('postselection uses grip-only SelPt')

def run(op,host,iterations,tolerance):
    validate(op)
    if iterations!=1:raise ValueError('grip placement requires one iteration')
    R=host['Rhino'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc;vp=doc.Views.ActiveView.ActiveViewport
    before=set(o.Id for o in doc.Objects);groups=set(g.Index for g in doc.Groups if not g.IsDeleted);plane=vp.GetConstructionPlane().Plane
    surface=host['_object_source'](op['surface'],tolerance);geometry=host['_object_source'](op['source'],tolerance)
    try:
        vp.SetConstructionPlane(G.Plane.WorldXY)
        guide=doc.Objects.AddSurface(surface);source=doc.Objects.Add(geometry);peer=None
        if op.get('peer'):peer=doc.Objects.AddPoint(G.Point3d(1.,1.,1.))
        if op.get('group'):doc.Groups.Add('Owned grip placement',[source]+([peer]if peer else[]))
        guide_definition=host['_interchange_brep_record'](doc.Objects.FindId(guide).Geometry,include_samples=False)
        doc.Objects.UnselectAll();obj=doc.Objects.FindId(source)
        if op.get('edit'):
            doc.Objects.Select(source,True);R.RhinoApp.RunScript('_EditPtOn',True);doc.Objects.UnselectAll();obj=doc.Objects.FindId(source)
        else:obj.GripsOn=True
        grips=obj.GetGrips()or[]
        if not grips or any(i>=len(grips)for i in op['selected']):raise ValueError('native grip index missing')
        if op.get('postselected')and set(op['selected'])!=set(range(len(grips))):raise ValueError('postselection must capture every grip')
        if not op.get('postselected'):
            for i in op['selected']:grips[i].Select(True)
            if op.get('parent'):doc.Objects.Select(source,True)
            if peer:doc.Objects.Select(peer,True)
        def snapshot():
            outputs=[]
            active_groups=sorted(g.Index for g in doc.Groups if not g.IsDeleted and g.Index not in groups)
            for o in doc.Objects:
                if o.Id in before or o.Id==guide:continue
                g=o.Geometry;row=dict(original=o.Id==source,peer=o.Id==peer,selected=bool(o.IsSelected(False)),group_count=o.GroupCount,groups=sorted(active_groups.index(i)for i in(o.GetGroupList()or[])if i in active_groups),grips_on=bool(o.GripsOn),grips=[dict(index=int(p.Index),point=host['_xyz'](p.CurrentLocation),selected=bool(p.IsSelected(False)))for p in(o.GetGrips()or[])])
                if isinstance(g,G.Curve):row['curve']=host['_nurbs_curve_definition'](g.ToNurbsCurve())
                elif isinstance(g,G.Brep):row['surface']=host['_nurbs_surface_definition'](g.Faces[0].UnderlyingSurface().ToNurbsSurface())
                elif isinstance(g,G.Mesh):row['mesh']=host['_polygon_mesh_value'](g)
                elif isinstance(g,G.Point):row['point']=host['_xyz'](g.Location)
                else:raise ValueError('unexpected grip owner output')
                outputs.append(row)
            return dict(outputs=outputs,group_count=sum(not g.IsDeleted and g.Index not in groups for g in doc.Groups),guide_selected=bool(doc.Objects.FindId(guide).IsSelected(False)),guide_unchanged=guide_definition==host['_interchange_brep_record'](doc.Objects.FindId(guide).Geometry,include_samples=False))
        seed=snapshot();o=op['options'];point=lambda p:'w'+','.join(repr(float(v))for v in p)
        flags='_Copy=_'+('Yes'if o.get('copy',True)else'No')+' _Rigid=_'+('Yes'if o.get('rigid',True)else'No')+' _IgnoreTrims=_No _Flip=_'+('Yes'if o.get('flip')else'No')+' '
        if not o.get('rigid',True):flags+='_ConstrainNormal=_'+('Yes'if o.get('constrain_normal')else'No')+' '
        macro='_-OrientOnSrf '+('_SelPt _Enter 'if op.get('postselected')else'')+point(op['base'])+' '+point(op['reference'])+" '_-SelID "+str(guide)+' '+flags
        for p in [dict(uv=op['uv'])]+op.get('batch',[]):
            q=surface.PointAt(*p['uv']);macro+=point(host['_xyz'](q))+' '+repr(float(p.get('scale',o.get('scale',1))))+' '+repr(float(p.get('rotation',o.get('rotation',0))))+' '
        macro+=('_Enter 'if o.get('copy',True)else'')+'_Cancel'
        marker='Owned grip placement '+str(host['System'].Guid.NewGuid());R.RhinoApp.WriteLine(marker)
        returned=R.RhinoApp.RunScript(macro,True);after=snapshot()
        return dict(before=seed,after=after,command_returned=bool(returned),macro=macro,history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[-1].strip()),0
    finally:
        for o in list(doc.Objects):
            if o.Id not in before:
                if o.GripsOn:o.GripsOn=False
                doc.Objects.Delete(o.Id,True)
        for g in list(doc.Groups):
            if not g.IsDeleted and g.Index not in groups:doc.Groups.Delete(g.Index)
        geometry.Dispose();surface.Dispose();vp.SetConstructionPlane(plane)
