import unittest

from tools.rhino_oracle.angle_cursor_probe import validate_request


class AngleCursorProbeTests(unittest.TestCase):
    def test_request_accepts_owned_free_and_constrained_mouse_cases(self):
        validate_request({
            "protocol_version": 1,
            "iterations": 1,
            "operations": [
                {"op": "angle_cursor_diagnostic", "id": "free", "angle": None, "aim": [10, 1, 0]},
                {"op": "angle_cursor_diagnostic", "id": "locked", "angle": 270, "aim": [10, 1, 0]},
            ],
        })

    def test_request_rejects_commands_duplicate_ids_and_nonfinite_values(self):
        base = {"op": "angle_cursor_diagnostic", "id": "case", "angle": 30, "aim": [10, 1, 0]}
        for operation in [
            dict(base, id="case;_Delete"),
            dict(base, angle=float("nan")),
            dict(base, aim=[float("inf"), 1, 0]),
            dict(base, script="_Delete"),
        ]:
            with self.assertRaises(ValueError):
                validate_request({"protocol_version": 1, "operations": [operation]})
        with self.assertRaises(ValueError):
            validate_request({"protocol_version": 1, "operations": [base, base]})


if __name__ == "__main__":
    unittest.main()
