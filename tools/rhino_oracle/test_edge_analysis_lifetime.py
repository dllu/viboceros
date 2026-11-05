"""Immutable public-command lifetime captures and bounded producer admission."""
import copy
import gzip
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from .edge_analysis_lifetime_cases import cases, runtime_cases, selection_cases, mode_cases
from .edge_analysis_lifetime_replay import FAMILY, FAMILIES
from .subd_edge_analysis_replay import HERE, read
from .structural_sdk import stage_helpers, validate_operations


class EdgeAnalysisLifetimeTests(unittest.TestCase):
    def test_frozen_recipes_and_producer_hashes(self):
        for family, recipes in zip(FAMILIES,(cases(),runtime_cases(),runtime_cases(),selection_cases(),mode_cases()),strict=True):
            self._frozen(family,recipes)

    def _frozen(self,family,recipes):
        path = HERE / 'fixtures' / (family+'.json')
        request = json.loads(path.read_bytes())
        validate_operations(request)
        self.assertEqual(request['operations'], recipes)
        native = read(family)
        self.assertEqual(native['provenance']['request_sha256'], hashlib.sha256(path.read_bytes()).hexdigest())
        self.assertEqual([r['id'] for r in native['results']], [op['id'] for op in recipes])
        for name, digest in native['provenance']['sources'].items():
            raw = gzip.decompress((HERE / 'observations/source_snapshots' / (digest+'.gz')).read_bytes())
            self.assertEqual(hashlib.sha256(raw).hexdigest(), digest, name)

    def test_native_commands_delete_and_restore_exact_sources(self):
        for op, row in zip(cases(), read(FAMILY)['results'], strict=True):
            v = row['value']
            self.assertTrue(v['success'], op['id'])
            self.assertTrue(all(c['success'] for c in v['commands']), op['id'])
            states = {s['action']: s['state'] for s in v['stages']}
            for action in ('Delete', 'RedoDelete'):
                self.assertEqual([s['present'] for s in states[action]['sources']], [i not in op['deleted'] for i in range(3)])
            for action in ('UndoDelete', 'Restore'):
                for a, b in zip(v['before']['sources'], states[action]['sources'], strict=True):
                    self.assertTrue(b['present'])
                    self.assertEqual(a['geometry'], b['geometry'])
            self.assertEqual(states['UndoMark']['points'], [])
            self.assertEqual(states['RedoMark']['points'], v['after']['points'])
            self.assertEqual(v['zoom_input']['errors'], [])
            self.assertIn('Undoing ZoomNaked', v['history'])
            self.assertIn('Redoing ZoomNaked', v['history'])

    def test_native_runtime_identity_and_activation_order_survive_history(self):
        for op,row in zip(runtime_cases(),read(FAMILIES[1])['results'],strict=True):
            v=row['value']
            self.assertTrue(v['success'])
            self.assertEqual(v['zoom_input']['errors'],[])
            self.assertEqual(len(v['after']['points']),8*(len(op['analyzed'])-len(op['removed'])))
            stages={s['action']:s for s in v['stages']}
            initial={s['kind']:s['serial'] for s in stages['Start']['runtime_serials']}
            restored={s['kind']:s['serial'] for s in stages['Restore']['runtime_serials']}
            self.assertEqual(initial,restored,op['id'])
            for phase in ('Undo'+op['edit'],'Restore'):
                self.assertEqual(stages[phase]['state']['runtime_order'],['brep','mesh','subd'])
            if op['edit']!='Delete':
                changed={s['kind']:s['serial'] for s in stages[op['edit']]['runtime_serials']}
                repeated={s['kind']:s['serial'] for s in stages['Redo'+op['edit']]['runtime_serials']}
                self.assertEqual(changed,repeated)
                for i,kind in enumerate(['brep','mesh','subd']):
                    self.assertEqual(changed[kind]!=initial[kind],i in op['targets'])
            self.assertEqual([s['token'] for s in v['zoom_input']['sent']],['_All','_Mark',''])

    def test_mode_controls_and_physical_zoom_preserve_preselection(self):
        for op,row in zip(selection_cases(),read('edge_analysis_panel_selection')['results'],strict=True):
            v=row['value']
            self.assertTrue(v['success'])
            selected=[i in op['targets'] for i in range(3)]
            for stage in v['stages']:
                if stage['action'] in ('Select','Mode','Zoom','UndoMark','RedoMark'):
                    self.assertEqual([s['selected'] for s in stage['state']['sources']],selected,op['id'])
                    self.assertEqual(stage['selection_states'],[2*int(s) for s in selected],op['id'])
            expected=0 if op['panel_mode']=='All' else 8*len(op['analyzed'])
            self.assertEqual(len(v['after']['points']),expected)
            if expected:
                self.assertTrue(all(s['acknowledged'] for s in v['zoom_input']['sent']))

    def test_mode_controls_preserve_persistence_and_remember_mode(self):
        labels={'All':'&All edges','Naked':'&Naked edges','NonManifold':'N&on-manifold\nedges'}
        def active(panel):
            return next(mode for mode,label in labels.items()
                        if any(c['label']==label and c['check']==1 for c in panel))
        remembered='All'
        for op,row in zip(mode_cases(),read('edge_analysis_modes')['results'],strict=True):
            v=row['value']
            self.assertTrue(v['success'],op['id'])
            self.assertEqual(active(v['opened_panel']),remembered,op['id'])
            steps=op.get('mode_sequence',[op['panel_mode']])
            mode_stages=[s for s in v['stages'] if s['action'].startswith('Mode')]
            self.assertEqual([active(s['panel']) for s in mode_stages],steps,op['id'])
            for stage in v['stages']:
                if stage['action']!='Start':
                    self.assertEqual(stage['selection_states'],[2,2,2],(op['id'],stage['action']))
            self.assertEqual(v['after']['points'],[])
            remembered=steps[-1]

    def test_failed_dispatcher_captures_are_retained_as_diagnostics(self):
        for family in ('edge_analysis_lifetime_input_diagnostic', 'edge_analysis_lifetime_dispatcher_diagnostic', 'edge_analysis_lifetime_reentrancy_diagnostic', 'edge_analysis_panel_selection_input_diagnostic', 'edge_analysis_panel_selection_timer_diagnostic', 'edge_analysis_panel_selection_host_timer_diagnostic'):
            data = read(family)
            self.assertIn('not successful geometry evidence', data['classification'])
            self.assertIn('owned edge Zoom input failed', data['driver_log'])
            self.assertNotIn('results', data)
            for name, digest in data['provenance']['sources'].items():
                raw = gzip.decompress((HERE / 'observations/source_snapshots' / (digest+'.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(raw).hexdigest(), digest, name)

    def test_startup_timeout_is_retained_separately(self):
        data=read('edge_analysis_mixed_ui_startup_diagnostic')
        self.assertIn('not successful geometry evidence',data['classification'])
        self.assertEqual(data['worker_progress'],'')
        self.assertIn('did not respond within 900 seconds',data['driver_log'])
        for name,digest in data['provenance']['sources'].items():
            raw=gzip.decompress((HERE/'observations/source_snapshots'/(digest+'.gz')).read_bytes())
            self.assertEqual(hashlib.sha256(raw).hexdigest(),digest,name)

    def test_validation_and_transitive_staging(self):
        recipes=cases()+runtime_cases()+selection_cases()+mode_cases()
        for op in recipes:
            for change in (dict(flat=1), dict(deleted=[True]), dict(analyzed=[0, 4]), dict(removed=[2, 2]), dict(extra=True), dict(runtime=1), dict(edit='DeleteAll'), dict(skip_zoom=1), dict(initial_mode='Invalid'), dict(mode_sequence=['Delete'])):
                with self.assertRaises(ValueError):
                    validate_operations(dict(operations=[dict(copy.deepcopy(op), **change)]))
        request = dict(protocol_version=1, iterations=1, operations=recipes)
        with tempfile.TemporaryDirectory() as folder:
            stage_helpers(request, HERE, Path(folder))
            subprocess.run([sys.executable, '-I', '-c', 'import sys;sys.path.insert(0,sys.argv[1]);import edge_analysis_lifetime_probe', folder], check=True, capture_output=True)
            asset = Path('fixtures/brep_direction/warped.3dm')
            self.assertEqual((Path(folder) / asset).read_bytes(), (HERE / asset).read_bytes())


if __name__ == '__main__':
    unittest.main()
