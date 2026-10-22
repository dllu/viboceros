"""Observe native commands with selected components in a new owned Rhino."""
try:
    from .mesh_component_unweld_command_cases import scene,validate
    from .mesh_channel_capture import capture,snapshot
except (ImportError, ValueError):
    from mesh_component_unweld_command_cases import scene,validate
    from mesh_channel_capture import capture,snapshot


def run(operation,host,iterations,tolerance):
    validate(operation)
    if iterations!=1:raise ValueError('one component command iteration required')
    case=scene(operation);accepted=[];stages={}
    R,S=host['Rhino'],host['System'];document=R.RhinoDoc.ActiveDoc
    def change(mesh):
        if not case['indices']:
            accepted.append(False);return
        identifier=document.Objects.AddMesh(mesh)
        if identifier==S.Guid.Empty:raise ValueError('could not create owned mesh source')
        try:
            document.Objects.UnselectAll()
            command='UnweldEdge'if case['kind']=='edge'else 'UnweldVertex'
            if case.get('prime_options'):
                before=R.RhinoApp.CommandHistoryWindowText
                R.RhinoApp.RunScript('_-'+command+' _ModifyNormals=_'+('Yes'if case['modify_normals']else 'No')+' _Enter',False)
                stages['priming_history']=R.RhinoApp.CommandHistoryWindowText[len(before):]

            obj=document.Objects.FindId(identifier)
            kind=(R.Geometry.ComponentIndexType.MeshTopologyEdge if case['kind']=='edge'
                  else R.Geometry.ComponentIndexType.MeshTopologyVertex)
            for i in case['indices']:
                if obj.SelectSubObject(R.Geometry.ComponentIndex(kind,i),True,True,False)==0:
                    raise ValueError('could not select owned mesh component')
            stages['document_before']=snapshot(obj.Geometry,host,case)
            history_before=R.RhinoApp.CommandHistoryWindowText
            # Nested RunScript may report false after completing the edit.
            macro='_-'+command+(''if case.get('prime_options')else ' _ModifyNormals=_'+('Yes'if case['modify_normals']else 'No'))+' _Enter'
            R.RhinoApp.RunScript(macro,False)
            stages['history']=R.RhinoApp.CommandHistoryWindowText[len(history_before):]
            obj=document.Objects.FindId(identifier)
            if obj is None:raise ValueError('component command removed its source')
            stages['document_after']=snapshot(obj.Geometry,host,case)
            mesh.CopyFrom(obj.Geometry)
            accepted.append(True)
        finally:
            document.Objects.UnselectAll()
            document.Objects.Delete(identifier,True)
    value,elapsed=capture(case,host,change)
    value['accepted']=accepted[0]
    if case.get('prime_options') and accepted[0]:
        value['recipe_mesh']=value['before']
        value['before']=stages['document_before']
        value['after_copy']=value['after']
        value['after']=stages['document_after']
        value['stages']=stages
    return value,elapsed
