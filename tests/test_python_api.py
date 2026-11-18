"""Exercise the Python package against the real headless application process."""
import os
import struct
import tempfile
import unittest
from pathlib import Path

from viboceros import CommandError, ENTER, ESCAPE, Pick, PickPoint, Session, SessionError


class PythonApiTests(unittest.TestCase):
    def setUp(self):
        self.model = Session()
        self.addCleanup(self.model.close)
        folder = tempfile.TemporaryDirectory()
        self.addCleanup(folder.cleanup)
        self.folder = Path(folder.name)

    def test_persistent_commands_and_ordered_selection(self):
        model = self.model
        self.assertIn("BooleanDifference", model.commands())
        self.assertIn("SetDisplayMode", model.commands())
        first = model.run("Line", (0, 0, 0))
        self.assertTrue(first.pending)
        self.assertIsNotNone(first.prompt)
        line, = model.run((5, 0, 0)).added
        self.assertEqual(model.object(line).bounds, ((0, 0, 0), (5, 0, 0)))
        a, b = model.run("Circle 0,0,0 3", "Circle 0,0,6 5").added
        self.assertEqual(model.select(b, a).objects, (b, a))
        model.select()
        loft = model.run("Loft", Pick(a), Pick(b), ENTER, ENTER)
        self.assertFalse(loft.pending)
        self.assertEqual(model.object(loft.added[0]).kind, "brep")
        self.assertEqual(model.run("Undo").deleted, loft.added)
        self.assertEqual(model.run("Redo").added, loft.added)
        with Session() as other:
            self.assertEqual(other.objects(), ())
            self.assertEqual(len(model.objects()), 4)

    def test_multistage_solid_boolean(self):
        model = self.model
        target, cutter = model.run("Box 0,0,0 10,10,0 5", "Box 6,3,-1 12,7,-1 7").added
        result = model.run("BooleanDifference", Pick(target), ENTER, Pick(cutter), ENTER)
        self.assertFalse(result.pending)
        self.assertEqual(len(result.added), 1)
        self.assertEqual(result.deleted, (target, cutter))
        self.assertEqual(model.objects()[0].bounds, ((0, 0, 0), (10, 10, 5)))

    def test_component_and_grip_identity(self):
        model = self.model
        box, = model.run("Box 0,0,0 8,5,0 3").added
        faces = [face for face in model.components(box) if face.kind == "brep_face"]
        self.assertEqual(len(faces), 6)
        self.assertEqual(model.select_components(faces[0]).components[0].index, faces[0].index)
        result = model.run("DupFaceBorder", Pick(faces[0]), select=[box])
        self.assertFalse(result.pending)
        self.assertEqual(len(result.added), 1)

        curve, = model.run("Curve Degree=3 0,0,0 1,3,0 4,1,0 7,2,0").added
        model.run("PointsOn", select=[curve])
        grips = model.grips(curve)
        self.assertEqual(len(grips), 4)
        self.assertEqual(model.select_grips(grips[1]).grips[0].index, 1)
        result = model.run("Move 0,0,0 0,0,1")
        self.assertEqual(result.modified, (curve,))
        self.assertEqual(model.grips(curve)[1].point, (1, 3, 1))

    def test_world_view_pick_and_error_recovery(self):
        model = self.model
        line, = model.run("Line 0,0,0 5,0,0").added
        view = next(view for view in model.viewports() if view.name == "Top")
        x, y = model.project((2.5, 0, 0), view="Top")
        self.assertTrue(view.rectangle[0] <= x <= view.rectangle[2])
        self.assertTrue(view.rectangle[1] <= y <= view.rectangle[3])
        self.assertEqual(model.run(PickPoint((2.5, 0, 0), view="Top")).selected, (line,))
        with self.assertRaises(CommandError) as raised:
            model.run("NoSuchVibocerosCommand", "Delete")
        self.assertEqual(raised.exception.result.processed, 1)
        self.assertEqual(len(model.objects()), 1)
        self.assertTrue(model.run("Line").pending)
        with self.assertRaises(SessionError):
            model.save(self.folder / "unfinished.3dm")
        self.assertFalse(model.run(ESCAPE).pending)

    def test_project_and_mesh_io(self):
        model = self.model
        object_id, = model.run("Box 0,0,0 8,5,0 3").added
        model.run('SetObjectName "Test model"', select=[object_id])
        model.run('NamedView Save "Inspection view"')
        project = self.folder / "test project.3dm"
        model.save(project)
        self.assertEqual(Path(model.info()["path"]), project)
        self.assertTrue(project.read_bytes().startswith(b"3D Geometry File Format"))
        stl = self.folder / "test mesh.stl"
        step = self.folder / "test faceted.stp"
        model.export(stl, ascii_stl=True)
        model.export(step)
        self.assertTrue(stl.read_bytes().startswith(b"solid"))
        self.assertTrue(step.read_bytes().startswith(b"ISO-10303-21"))
        model.new()
        self.assertEqual(model.objects(), ())
        model.open(project)
        self.assertEqual(model.object(object_id).name, "Test model")
        self.assertIn("Inspection view", model.run("NamedView List").messages[-1])
        self.assertEqual(len(model.import_file(stl).added), 1)
        facets = model.import_file(step).added
        self.assertTrue(facets)
        self.assertTrue(all(obj.kind == "mesh" for obj in model.objects(*facets)))
        self.assertEqual(len(model.import_file(project).added), 1)

    def test_native_step_round_trip(self):
        model = self.model
        if not model.info()["native_smlib"]:
            self.skipTest("requires native-smlib")
        model.run("Box 0,0,0 8,5,0 3")
        step = self.folder / "native model.step"
        model.export(step, native_step=True)
        model.new()
        restored, = model.import_file(step, native_step=True).added
        self.assertEqual(model.object(restored).kind, "brep")
        self.assertEqual(model.object(restored).bounds, ((0, 0, 0), (8, 5, 3)))

    @unittest.skipUnless(os.environ.get("VIBOCEROS_TEST_GPU") == "1", "requires a wgpu adapter")
    def test_shaded_png_from_multiple_views(self):
        model = self.model
        size = (303, 247)
        blank = model.screenshot(self.folder / "empty.png", size=size).read_bytes()
        model.run("Box 0,0,0 8,5,0 3")
        captures = set()
        for view in ("Top", "Perspective", "Back"):
            png = model.screenshot(self.folder / f"{view}.png", view=view, size=size).read_bytes()
            self.assertEqual(png[:8], b"\x89PNG\r\n\x1a\n")
            self.assertEqual(struct.unpack(">II", png[16:24]), size)
            self.assertNotEqual(png, blank)
            captures.add(png)
        self.assertEqual(len(captures), 3)


if __name__ == "__main__":
    unittest.main()
