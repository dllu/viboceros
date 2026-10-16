"""Compare preset placements and the public controls used by the native capture."""
import argparse
import json
from pathlib import Path
from .orient_surface_comparison import comparison as model_comparison


def dialog_matches(state, options, which='after'):
    try:
        if state['applied'] is not True or state['errors'] != []:
            return False
        rows = state[which]
        normalize = lambda s: s.replace('&', '').replace(' ', '').lower()
        def button(label):
            found = [r for r in rows if r['kind'] == 'Button' and normalize(r['text']) == label]
            if len(found) != 1:
                raise ValueError('ambiguous control')
            return found[0]['checked'] == 1
        prompts = sorted([r for r in rows if r['kind'] == 'Button' and normalize(r['text']) == 'prompt'], key=lambda r: r['rect'][0])
        edits = sorted([r for r in rows if 'edit' in r['kind'].lower()], key=lambda r: (r['rect'][0], r['rect'][1]))
        if len(prompts) != 2 or len(edits) != 4:
            return False
        uniform = options.get('uniform', True)
        scales = options.get('axes', [options.get('scale', 1)] * 3)
        expected = list(scales) + [options.get('rotation', 0)]
        indices = [0, 3] if uniform else range(4)
        return (button('uniform') == uniform and button('rigid') == options.get('rigid', True)
                and all((p['checked'] == 1) == options.get(k, False) for p, k in zip(prompts, ('scale_prompt', 'rotation_prompt')))
                and all(abs(float(edits[i]['text']) - expected[i]) <= 1e-12 for i in indices))
    except (KeyError, TypeError, ValueError, IndexError):
        return False


def comparison(request, local, native):
    result = model_comparison(request, local, native)
    recipes = {op['id']: op for op in request['operations']}
    rows = {r['id']: r['value'] for r in native['results']}
    for row in result['results']:
        op, value = recipes[row['id']], rows[row['id']]
        matched = dialog_matches(value.get('dialog', {}), op['options'])
        matched &= 'Unknown command' not in value.get('history', '')
        if op.get('inherit'):
            matched &= dialog_matches(value.get('dialog', {}), op['setup'], 'before')
        if 'setup' in op:
            matched &= dialog_matches((value.get('setup') or {}).get('dialog', {}), op['setup'])
            matched &= (value.get('setup') or {}).get('command_returned') is False
        row['dialog_options_match'] = bool(matched)
        row['within_1e_minus_6'] &= bool(matched)
    result['matched_cases'] = sum(r['within_1e_minus_6'] for r in result['results'])
    return result


if __name__ == '__main__':
    p = argparse.ArgumentParser(description=__doc__)
    for name in ('request', 'local', 'native', 'output'):
        p.add_argument(name, type=Path)
    a = p.parse_args()
    a.output.write_text(json.dumps(comparison(json.loads(a.request.read_text()), json.loads(a.local.read_text()), json.loads(a.native.read_text())), indent=2) + '\n')
