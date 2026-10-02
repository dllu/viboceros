"""Native input diagnostics must release callbacks on every exit path."""
import unittest
from types import SimpleNamespace
from unittest.mock import Mock

from .shrink_face_input import _input_hooks


class Event:
    def __init__(self, *, add_error=False, remove_error=False):
        self.handlers=[];self.add_error=add_error;self.remove_error=remove_error
        self.removed=[]

    def __iadd__(self, handler):
        self.handlers.append(handler)
        if self.add_error:raise RuntimeError('registration failed')
        return self

    def __isub__(self, handler):
        self.removed.append(handler)
        self.handlers.remove(handler)
        if self.remove_error:raise RuntimeError('removal failed')
        return self


class ShrinkInputHookTests(unittest.TestCase):
    def test_normal_and_exceptional_command_exit_release_every_hook(self):
        for fail in (False,True):
            with self.subTest(fail=fail):
                timer=Mock();mouse=SimpleNamespace(Enabled=False)
                events=[Event() for _ in range(4)];handlers=[(event,object()) for event in events]
                def run():
                    with _input_hooks(timer,mouse,handlers):
                        self.assertTrue(mouse.Enabled)
                        self.assertTrue(all(event.handlers for event in events))
                        if fail:raise RuntimeError('command failed')
                if fail:
                    with self.assertRaisesRegex(RuntimeError,'command failed'):run()
                else:run()
                self.assertFalse(mouse.Enabled);self.assertTrue(all(not event.handlers for event in events))
                timer.Stop.assert_called_once();timer.Dispose.assert_called_once()

    def test_partial_registration_and_timer_start_failures_are_cleaned(self):
        for failure in ('registration','start'):
            with self.subTest(failure=failure):
                timer=Mock();mouse=SimpleNamespace(Enabled=False)
                events=[Event(),Event(add_error=failure=='registration'),Event()]
                handlers=[(event,object()) for event in events]
                if failure=='start':timer.Start.side_effect=RuntimeError('start failed')
                with self.assertRaisesRegex(RuntimeError,'failed'):
                    with _input_hooks(timer,mouse,handlers):self.fail('body must not run')
                self.assertFalse(mouse.Enabled);self.assertTrue(all(not event.handlers for event in events))
                timer.Stop.assert_called_once();timer.Dispose.assert_called_once()

    def test_cleanup_continues_after_stop_and_event_removal_failures(self):
        timer=Mock();timer.Stop.side_effect=RuntimeError('stop failed')
        mouse=SimpleNamespace(Enabled=False)
        events=[Event(remove_error=True),Event()];handlers=[(event,object()) for event in events]
        with self.assertRaisesRegex(ValueError,'stop failed.*removal failed'):
            with _input_hooks(timer,mouse,handlers):pass
        self.assertFalse(mouse.Enabled);self.assertTrue(all(not event.handlers for event in events))
        self.assertTrue(all(event.removed for event in events));timer.Dispose.assert_called_once()


if __name__=='__main__':unittest.main()
