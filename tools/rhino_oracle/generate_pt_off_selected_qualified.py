"""Confirmed Redo witness and bounded public object getter input."""
from .capture_family import capture
from .generate_pt_off_selected import request as original


def request():
    query=original()
    query['operations'] += [dict(op='pt_off_selected',id='curve-control-'+pattern,
                                kind='curve',mode='control',pattern=pattern)
                            for pattern in ['owner-peer','off-owner','postselect','owner-hidden-peer']]
    return query


if __name__=='__main__':
    capture('pt_off_selected_qualified',request(),[
        'generate_pt_off_selected_qualified.py','generate_pt_off_selected.py',
        'pt_off_selected_probe.py','hide_pt_input.py','join_probe.py',
        'merge_edges_probe.py','rhino_worker.py','client.py','structural_sdk.py'],
        'VibocerosOraclePtOffSelectedQualified20261029',timeout=450)
