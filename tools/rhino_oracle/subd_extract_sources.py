"""Public 3DM source archives keep native component orientation in Rust replays."""
if __package__:
    from .subd_extract_cases import all_cases
    from .subd_join_probe import source
else:
    from subd_extract_cases import all_cases
    from subd_join_probe import source


def validate(op):
    if (not isinstance(op,dict) or set(op)!={'op','id','scene'} or op['op']!='subd_extract_sources'
            or not isinstance(op['scene'],str) or op['scene'] not in all_cases() or op['id']!=op['scene']):
        raise ValueError('fixed owned SubD source archive required')


def run(op,host,iterations,tolerance):
    validate(op)
    if iterations!=1 or list(host['Rhino'].RhinoDoc.ActiveDoc.Objects):raise ValueError('empty owned document required')
    from subd_box_probe import subd_value
    records=[]
    for net in all_cases()[op['scene']]['sources']:
        subd=source(net,host);model=host['Rhino'].FileIO.File3dm()
        try:
            model.Objects.AddSubD(subd)
            records.append(dict(subd=subd_value(subd,host),archive=[int(b) for b in model.ToByteArray()]))
        finally:model.Dispose();subd.Dispose()
    return records,0
