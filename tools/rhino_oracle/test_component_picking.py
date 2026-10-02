import subprocess
import tempfile
from pathlib import Path
import unittest
from unittest.mock import patch
from .component_picking import ComponentPicker


class ComponentPickingTests(unittest.TestCase):
    def test_component_finish_keys_are_scoped_and_released_on_failure(self):
        for value,key in [('Enter','Return'),('Cancel','Escape')]:
            for fail in (False,True):
                calls=[]
                def run(command,**kwargs):
                    calls.append(command)
                    if fail and command[1]=='keydown':raise subprocess.CalledProcessError(1,command)
                with patch('tools.rhino_oracle.component_picking.subprocess.run',side_effect=run),patch('tools.rhino_oracle.component_picking.time.sleep'):
                    if fail:
                        with self.assertRaises(subprocess.CalledProcessError):ComponentPicker().send_input('@component-finish:owned:'+value,'1','1','123')
                    else:ComponentPicker().send_input('@component-finish:owned:'+value,'1','1','123')
                self.assertEqual(calls[0],['xdotool','windowactivate','--sync','123'])
                self.assertEqual(calls[-1],['xdotool','keyup',key])
        with patch('tools.rhino_oracle.component_picking.subprocess.run') as run:
            with self.assertRaises(ValueError):ComponentPicker().send_input('@component-finish:owned:Delete','1','1','123')
            run.assert_not_called()

    def test_modifier_sequence_activates_owned_window_and_releases_on_failure(self):
        for kind in ('click','window'):
            for modifiers, keys in [('plain',[]),('ctrl',['ctrl']),('shift',['shift']),('sub',['ctrl','shift']),('alt',['alt'])]:
                for fail in (False,True):
                    marker='@component-%s:owned:1:%s'%(kind,modifiers)
                    if kind=='window': marker+=':30:40'
                    calls=[]
                    def run(command, **kwargs):
                        calls.append(command)
                        self.assertEqual(kwargs,dict(check=True,timeout=10))
                        if fail and 'mousedown' in command: raise subprocess.CalledProcessError(1,command)
                    with patch('tools.rhino_oracle.component_picking.subprocess.run',side_effect=run),patch('tools.rhino_oracle.component_picking.time.sleep'):
                        if fail:
                            with self.assertRaises(subprocess.CalledProcessError): ComponentPicker().send_input(marker,'10','20','123')
                        else: self.assertTrue(ComponentPicker().send_input(marker,'10','20','123'))
                    self.assertEqual(calls[0],['xdotool','windowactivate','--sync','123'])
                    self.assertIn(['xdotool','mouseup','1'],calls)
                    self.assertEqual([c[-1] for c in calls if c[1]=='keyup'],list(reversed(keys)))

    def test_malformed_markers_never_send_input(self):
        with patch('tools.rhino_oracle.component_picking.subprocess.run') as run:
            for marker in ['@component-click:owned:0:other','@component-click:owned:0:ctrl:1:2','@component-window:owned:0:plain','@component-window:owned:0:sub:-1:2','@component-click:owned:100:plain','@component-key:owned:0:_Delete']:
                with self.assertRaises(ValueError): ComponentPicker().send_input(marker,'1','2','123')
            run.assert_not_called()

    def test_cleanup_attempts_every_release_even_if_an_earlier_release_fails(self):
        calls=[]
        def run(command,**kwargs):
            calls.append(command)
            if command[1] in ('mouseup','keyup'):raise subprocess.CalledProcessError(1,command)
        with patch('tools.rhino_oracle.component_picking.subprocess.run',side_effect=run):
            with self.assertRaises(subprocess.CalledProcessError):ComponentPicker().send_input('@component-click:owned:0:sub','10','20','123')
        self.assertEqual(calls[-3:],[['xdotool','mouseup','1'],['xdotool','keyup','shift'],['xdotool','keyup','ctrl']])

    def test_step_identifiers_deliver_repeated_gestures_only_to_owned_windows(self):
        names=['@component-click:owned:0:plain','@component-click:owned:1:plain']
        with tempfile.TemporaryDirectory() as tmp,patch('tools.rhino_oracle.group_picking.time.monotonic',return_value=10),patch('tools.rhino_oracle.group_picking._rhino_window_for_pids',return_value=None) as window,patch.object(ComponentPicker,'send_input',return_value=True) as send:
            job=Path(tmp);(job/'worker-progress.log').write_text(''.join('PICK %s 10 20\n'%name for name in names))
            picker=ComponentPicker();picker.ready=dict.fromkeys(names,0)
            picker(job,set());window.assert_not_called();send.assert_not_called()
            picker(job,{456});send.assert_not_called()
            window.return_value='123';picker(job,{456})
            self.assertEqual([call.args for call in send.call_args_list],[(name,'10','20','123') for name in names])
            picker(job,{456});self.assertEqual(send.call_count,2)

    def test_prompt_keys_are_whitelisted_and_scoped(self):
        with patch('tools.rhino_oracle.component_picking.subprocess.run') as run:
            for key in ('None','Undo'):
                ComponentPicker().send_input('@component-key:owned:0:'+key,'1','1','123')
                self.assertEqual(run.call_args_list[-2].args[0],['xdotool','windowactivate','--sync','123','type','--clearmodifiers',key])
                self.assertEqual(run.call_args_list[-1].args[0],['xdotool','windowactivate','--sync','123','key','--clearmodifiers','Return'])


if __name__=='__main__':unittest.main()
