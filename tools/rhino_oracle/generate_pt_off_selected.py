"""Freeze point-display owner selection and grip selection behavior."""
from .capture_family import capture
from .pt_off_selected_probe import KINDS


def request():
    scenes=[(kind,mode,pattern) for kind in KINDS
            for mode in (['control'] if kind in ['surface','mesh'] else ['control','edit'])
            for pattern in ['owner','grip']]
    scenes += [('curve','control',pattern) for pattern in
               ['both-owners','both-grips','none','peer','mixed','group-owner','group-grip']]
    return dict(protocol_version=1,iterations=1,
                tolerance=dict(absolute=1e-7,relative=1e-12,angular=1e-10),
                operations=[dict(op='pt_off_selected',id=kind+'-'+mode+'-'+pattern,
                                 kind=kind,mode=mode,pattern=pattern) for kind,mode,pattern in scenes])


if __name__=='__main__':
    capture('pt_off_selected',request(),[
        'generate_pt_off_selected.py','pt_off_selected_probe.py','join_probe.py',
        'merge_edges_probe.py','rhino_worker.py','client.py','structural_sdk.py'],
        'VibocerosOraclePtOffSelected20261029',timeout=450)
