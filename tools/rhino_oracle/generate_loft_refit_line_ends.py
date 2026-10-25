"""Straight Refit references with closed lofts and singular point ends."""
import copy
from .capture_family import capture
from .generate_loft_refit_lines import request as line_request


def request():
    query = line_request()
    operations = []
    for original in query['operations']:
        if not original['id'].startswith('loft-refit-lines-line-') or original['refit_tolerance'] != .001:
            continue
        for kind in ['closed', 'start', 'end', 'both']:
            if kind != 'closed' and original['style'] == 'uniform':
                continue
            op = copy.deepcopy(original)
            op['id'] = op['id'] + '-' + kind
            if kind == 'closed':
                op['closed'] = True
            if kind in ('start', 'both'):
                op['start_point'] = [-2., -3., -2.]
            if kind in ('end', 'both'):
                op['end_point'] = [6., 4., 8.]
            operations.append(op)
    query['operations'] = operations
    return query


if __name__ == '__main__':
    capture('loft_refit_line_ends', request(), ['generate_loft_refit_line_ends.py',
        'generate_loft_refit_lines.py', 'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleLoftRefitLineEnds20261025')
