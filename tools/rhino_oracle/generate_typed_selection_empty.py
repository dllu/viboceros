"""Retain native no-match selection and preselection behavior separately."""
from .capture_family import capture
from .typed_selection_probe import COMMANDS


def request():
    return dict(protocol_version=1,iterations=1,
                tolerance=dict(absolute=1e-7,relative=1e-12,angular=1e-10),
                operations=[dict(op='typed_selection',id=command+'-'+scenario,
                                 command=command,scenario=scenario)
                            for command in COMMANDS
                            for scenario in ['no-matches','no-matches-preselected']])


if __name__ == '__main__':
    capture('typed_selection_empty',request(),[
        'generate_typed_selection_empty.py','typed_selection_probe.py',
        'join_probe.py','merge_edges_probe.py','rhino_worker.py','client.py','structural_sdk.py'],
        'VibocerosOracleTypedSelectionEmpty20261029',timeout=300)
