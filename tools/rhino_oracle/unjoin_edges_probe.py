"""Public Brep.UnjoinEdges on independently exported, owned source topology."""
import re


def validate(operation):
    if (not isinstance(operation, dict) or set(operation) != {'op','id','source','edges'}
            or operation.get('op') != 'brep_unjoin_edges'
            or not isinstance(operation.get('id'),str) or re.match(r'^[A-Za-z0-9_.-]{1,100}\Z',operation['id']) is None
            or not isinstance(operation.get('source'),dict)):
        raise ValueError('invalid edge separation fixture')
    path = operation['source'].get('artifact_path')
    edges = operation['edges']
    if (not isinstance(path,(str,type(u''))) or not path or not isinstance(edges,list)
            or len(edges)>100000 or any(type(edge) is not int or edge<0 for edge in edges)):
        raise ValueError('edge separation requires an owned source and nonnegative edge indices')
    return path,edges


def run(operation,tolerance,host):
    path,edges = validate(operation)
    Rhino,System = host['Rhino'],host['System']
    if path.startswith('/'): path = 'Z:'+path.replace('/','\\')
    model = Rhino.FileIO.File3dm.Read(path)
    if model is None: raise ValueError('cannot read edge separation source')
    source,working,results = None,None,[]
    try:
        try:
            entries=list(model.Objects)
            if len(entries)!=1: raise ValueError('edge separation needs one source')
            source=entries[0].Geometry.Duplicate()
        finally: model.Dispose()
        if not isinstance(source,Rhino.Geometry.Brep) or not source.IsValid:
            raise ValueError('invalid edge separation source')
        if any(edge>=source.Edges.Count for edge in edges): raise ValueError('edge outside source')
        record=lambda brep: host['_interchange_brep_record'](brep,include_samples=False)
        before=record(source)
        working=source.DuplicateBrep()
        result=working.UnjoinEdges(System.Array[System.Int32](edges))
        if result is not None: results=list(result)
        if any(not brep.IsValid for brep in results): raise ValueError('invalid separated B-rep')
        if record(source)!=before: raise ValueError('separation mutated independent source')
        return dict(before=before,after=None if result is None else [record(brep) for brep in results],working=record(working)),0
    finally:
        for brep in results: brep.Dispose()
        if working is not None: working.Dispose()
        if source is not None: source.Dispose()
