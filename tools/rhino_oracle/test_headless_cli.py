"""Live CLI modes must use their own X server on Linux."""

import unittest

from .__main__ import headless_wrapper_argv


class HeadlessCliTests(unittest.TestCase):
    def test_live_modes_route_through_xvfb_wrapper(self):
        for mode in ("rhino", "compare"):
            argv = [mode, "fixture.json", "--timeout", "300"]
            command = headless_wrapper_argv(mode, argv, {"DISPLAY": ":0"}, "linux")
            self.assertIsNotNone(command)
            self.assertTrue(command[0].endswith("/run_headless.sh"))
            self.assertEqual(command[1:], argv)

    def test_replay_and_isolated_or_explicit_visible_runs_do_not_reexec(self):
        self.assertIsNone(headless_wrapper_argv("replay", ["replay", "fixture.json"], {}, "linux"))
        self.assertIsNone(headless_wrapper_argv("rhino", ["rhino", "fixture.json"],
                                                 {"VIBOCEROS_ORACLE_HEADLESS": "1"}, "linux"))
        self.assertIsNone(headless_wrapper_argv("rhino", ["rhino", "fixture.json"],
                                                 {"VIBOCEROS_RHINO_VISIBLE": "1"}, "linux"))
        self.assertIsNone(headless_wrapper_argv("rhino", ["rhino", "fixture.json"], {}, "darwin"))


if __name__ == "__main__":
    unittest.main()
