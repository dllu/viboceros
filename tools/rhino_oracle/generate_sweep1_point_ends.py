"""Sweep1 SDK point ends with complete surface definitions."""
from .capture_family import capture


def request(command=False, refit=True):
    operations = []
    for name, stations, widths, start, end, blend in [
        ('start', [5.], [2.], [0.,0.,0.], None, 0),
        ('end', [0.], [2.], None, [0.,0.,5.], 0),
        ('both', [2.5], [2.], [0.,0.,0.], [0.,0.,5.], 0),
        ('both-global', [2.5], [2.], [0.,0.,0.], [0.,0.,5.], 1),
        ('offset-end', [0.], [2.], None, [1.,0.,5.], 0),
        ('multiple', [1.,4.], [2.,1.], [0.,0.,0.], [0.,0.,5.], 0),
    ]:
        op = dict(op='sweep1',id=name,
            rail=dict(type='line',start=[0.,0.,0.],end=[0.,0.,5.]),
            sections=[dict(type='line',start=[0.,0.,t],end=[w,0.,t])
                      for t,w in zip(stations,widths)],
            parameters=stations,blend=blend,inspect_definition=True)
        if start is not None:op['start_point']=start
        if end is not None:op['end_point']=end
        if command:
            op.update(id=name+'-command',command=True,refit_rail=refit)
        operations.append(op)
    return dict(protocol_version=1,iterations=1,
        tolerance=dict(absolute=1e-7,relative=1e-12,angular=1e-10),operations=operations)


if __name__=='__main__':
    capture('sweep1_point_ends',request(),[
        'generate_sweep1_point_ends.py','rhino_worker.py','client.py','structural_sdk.py'],
        'VibocerosOracleSweepPointEnds20261027',timeout=240)
