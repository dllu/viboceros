"""Observe selected native welding commands in an owned document/profile."""
try:
    from .mesh_component_weld_cases import CASES,validate as validate_base
    from . import mesh_component_weld_followup_cases as followup
    from . import mesh_component_weld_holdout_cases as holdout
    from .mesh_channel_capture import capture,snapshot
except (ImportError, ValueError):
    from mesh_component_weld_cases import CASES,validate as validate_base
    import mesh_component_weld_followup_cases as followup
    import mesh_component_weld_holdout_cases as holdout
    from mesh_channel_capture import capture,snapshot


def validate(operation):
    (holdout.validate if operation.get('op')=='mesh_component_weld_holdout'else followup.validate if operation.get('op')=='mesh_component_weld_followup'else validate_base)(operation)


def run(operation,host,iterations,tolerance):
    extra=operation.get('op')=='mesh_component_weld_followup'
    held=operation.get('op')=='mesh_component_weld_holdout'
    validate(operation)
    if iterations!=1:raise ValueError('one component welding iteration required')
    case=(holdout.CASES if held else followup.CASES if extra else CASES)[operation['case']];stages={}
    R,S=host['Rhino'],host['System'];document=R.RhinoDoc.ActiveDoc
    def change(mesh):
        if not mesh.IsValid:
            stages['invalid_source']=True;return
        if case['kind']=='whole':
            from math import pi
            mesh.Weld(pi);return
        if not case['indices']:return
        identifier=document.Objects.AddMesh(mesh)
        if identifier==S.Guid.Empty:raise ValueError('could not create owned mesh source')
        try:
            document.Objects.UnselectAll()
            obj=document.Objects.FindId(identifier)
            kind=(R.Geometry.ComponentIndexType.MeshTopologyEdge if case['kind']=='edge'
                  else R.Geometry.ComponentIndexType.MeshTopologyVertex)
            for i in case['indices']:
                if obj.SelectSubObject(R.Geometry.ComponentIndex(kind,i),True,True,False)==0:
                    raise ValueError('could not select owned mesh component')
            stages['document_before']=snapshot(obj.Geometry,host,case)
            command='WeldEdge'if case['kind']=='edge'else 'WeldVertices'
            R.RhinoApp.RunScript('_-'+command+' _Enter',False)
            obj=document.Objects.FindId(identifier)
            if obj is None:raise ValueError('component command removed its source')
            stages['document_after']=snapshot(obj.Geometry,host,case)
            mesh.CopyFrom(obj.Geometry)
        finally:
            document.Objects.UnselectAll()
            document.Objects.Delete(identifier,True)
    value,elapsed=capture(case,host,change)
    value['invalid_source']=bool(stages.get('invalid_source'))
    value['invoked']=bool(case['indices'])and not value['invalid_source']
    if value['invoked']:
        value['recipe_mesh']=value['before'];value['before']=stages['document_before']
        value['after_copy']=value['after'];value['after']=stages['document_after']
        value['stages']=stages
    return value,elapsed
