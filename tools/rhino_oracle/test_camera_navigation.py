"""Failure cleanup for mouse input delivered to the owned oracle display."""

import subprocess
import unittest
from unittest.mock import patch

from .camera_navigation import CameraNavigator


class CameraNavigationTests(unittest.TestCase):
    def test_input_failure_releases_the_mouse_button(self):
        navigator = CameraNavigator({"operations": [{"id": "drag", "mouse_drag": [60, 0]}]})
        for failure_call in (1, 4):
            calls = []

            def run(command, **_kwargs):
                calls.append(command)
                if len(calls) == failure_call:
                    raise subprocess.TimeoutExpired(command, 10)

            with self.subTest(failure_call=failure_call), \
                    patch("tools.rhino_oracle.camera_navigation.time.sleep"), \
                    patch("tools.rhino_oracle.camera_navigation.subprocess.run", side_effect=run):
                with self.assertRaises(subprocess.TimeoutExpired):
                    navigator.send_input("drag", "400", "300", "owned-window")
                self.assertEqual(calls[-1], ["xdotool", "mouseup", "3"])
                self.assertFalse(any("Escape" in command for command in calls))


if __name__ == "__main__":
    unittest.main()
