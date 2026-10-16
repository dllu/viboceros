import copy
import json
from pathlib import Path
from unittest import TestCase
from .orient_surface_options_probe import validate
from .orient_surface_options_comparison import comparison
BASE = Path(__file__).parent

class SurfaceOptionsTests(TestCase):
    def test_authored_dialog_inputs_validate_and_invalid_inputs_are_rejected(self):
        request = json.loads((BASE/'fixtures/orient_surface_options.json').read_text())
        for op in request['operations']:
            validate(op)
        original = request['operations'][0]
        for field, value in [('axes', [1,2]), ('axes', [True,2,3]), ('axes', [1,2,float('nan')]), ('uniform', 1), ('scale_prompt', 'Yes'), ('rotation_prompt', None), ('unknown', True)]:
            op = copy.deepcopy(original);op['options'][field] = value
            with self.assertRaises(ValueError, msg=field):
                validate(op)
        for field, value in [('inherit', 'yes'), ('setup', {'axes': [1,2]}), ('options', None), ('options', []), ('options', {'uniform': True, 'scale': 2, 'axes': [2,3,4]}), ('inherit', True)]:
            op = copy.deepcopy(original);op[field] = value
            with self.assertRaises(ValueError, msg=field):
                validate(op)

    def test_retained_models_and_owned_dialog_controls_detect_tampering(self):
        request = json.loads((BASE/'fixtures/orient_surface_options.json').read_text())
        read = lambda suffix: json.loads((BASE/'observations'/('orient_surface_options_'+suffix+'_20261016.json')).read_text())
        local, native = read('viboceros'), read('rhino')
        expected = comparison(request, local, native)
        self.assertEqual(expected['matched_cases'], 28)
        self.assertEqual(expected, read('comparison'))
        for kind in ('point','applied','errors','field','prompt','setup','inherited_before','unknown'):
            changed = copy.deepcopy(native)
            i = 22 if kind in ('setup','inherited_before') else 0
            v = changed['results'][i]['value']
            if kind == 'point':v['outputs'][0]['points'][1][0] += .1
            elif kind == 'applied':v['dialog']['applied'] = False
            elif kind == 'errors':v['dialog']['errors'].append('validation failure')
            elif kind == 'setup':v['setup']['dialog']['applied'] = False
            elif kind == 'unknown':v['history'] += '\nUnknown command: _Scale'
            else:
                rows = v['dialog']['before' if kind == 'inherited_before' else 'after']
                row = next(r for r in rows if r['kind'] == ('Button' if kind == 'prompt' else 'Edit') and (kind != 'prompt' or r['text'] == 'Prompt'))
                if kind == 'prompt':row['checked'] = 1-row['checked']
                else:row['text'] = '100'
            self.assertEqual(comparison(request, local, changed)['matched_cases'],27,kind)
