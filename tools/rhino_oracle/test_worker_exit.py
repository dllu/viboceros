"""Worker disappearance is terminal; observation silence is not."""
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
from .client import _owned_worker_exited


class WorkerExitTests(unittest.TestCase):
    def test_only_started_observed_workers_are_checked_without_waiting(self):
        with tempfile.TemporaryDirectory() as directory:
            progress = Path(directory)/'worker-progress.log'
            with patch('tools.rhino_oracle.client._wait_for_process_exit',return_value=True) as exited:
                self.assertFalse(_owned_worker_exited({12},progress))
                progress.write_text('worker: started\n')
                self.assertFalse(_owned_worker_exited(set(),progress))
                exited.assert_not_called()
                self.assertTrue(_owned_worker_exited({12},progress))
                exited.assert_called_once_with({12},0.)

    def test_a_live_worker_is_not_restarted_because_progress_is_unchanged(self):
        with tempfile.TemporaryDirectory() as directory:
            progress = Path(directory)/'worker-progress.log'
            progress.write_text('worker: started\n')
            with patch('tools.rhino_oracle.client._wait_for_process_exit',return_value=False):
                for _ in range(3): self.assertFalse(_owned_worker_exited({12},progress))


if __name__=='__main__': unittest.main()
