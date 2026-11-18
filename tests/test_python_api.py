"""Exercise the Python package against the real headless application process."""
import os
import struct
import tempfile
import unittest
from pathlib import Path

from viboceros import (
    ActivateView, Camera, CommandError, ENTER, ESCAPE, Key, NurbsSurface, Pick,
    PickPoint, Session, SessionError, SurfacePatch, SurfaceSample, TrimCurve, TypeText, WidgetAction,
)


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
        measure = model.run("Volume", select=result.added).messages[-1]
        self.assertAlmostEqual(float(measure.rsplit(" ", 1)[1]), 420.0, places=9)

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
        with self.assertRaisesRegex(SessionError, "geometry has changed"):
            model.select_grips(grips[1])

    def test_dialog_controls(self):
        model = self.model
        line, = model.run("Line 0,0,0 4,0,0").added
        instance, = model.run("Block 0,0,0 Inspection", select=[line]).added
        model.select()
        model.run("BlockManager")
        row = next(widget for widget in model.widgets() if widget.label == "Inspection")
        self.assertTrue(row.clickable)
        result = model.run(WidgetAction(row), WidgetAction("Select top-level instances"))
        self.assertEqual(result.selected, (instance,))
        field = next(widget for widget in model.widgets() if widget.value == "Inspection Copy")
        duplicate = model.run(
            WidgetAction(field, "focus"), Key("A", command=True),
            TypeText("Inspection alternate"), WidgetAction("Duplicate definition"),
        )
        self.assertIn("Duplicated definition as 'Inspection alternate'", duplicate.messages)

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
        object_id, = model.command("Box", (0, 0, 0), (8, 5, 0), 3).added
        model.command("SetObjectName", "Test model", select=[object_id])
        model.run("NamedView Save Inspection view")
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
        self.assertEqual(model.run("NamedView List").messages[-1], "Named views: Inspection view")
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

    def test_exact_surface_patches_and_jets(self):
        model = self.model
        knots = [0, 0, 0, 1, 1, 1]
        # The tensor quadratic is S(u,v)=(u,v,u²+uv+v²).
        net = [[(u/2, v/2, int(u == 2) + u*v/4 + int(v == 2)) for v in range(3)] for u in range(3)]
        support = NurbsSurface(2, 2, knots, knots, net)
        left, right = model.add_surface_patches(
            support, SurfacePatch(((0, .5), (0, 1)), "Left", (70, 32, 120)),
            SurfacePatch(((.5, 1), (0, 1)), "Right"),
        )
        self.assertEqual(model.surface_definition(left).domain, ((0, .5), (0, 1)))
        a, = model.surface_jets(left, SurfaceSample((.5, .25), side_u="left"))
        b, = model.surface_jets(right, (.5, .25))
        self.assertEqual(a, b)
        self.assertEqual(a.point, (.5, .25, .4375))
        self.assertEqual(a.du, (1., 0., 1.25))
        self.assertEqual((a.duu, a.duv, a.dvv), ((0., 0., 2.), (0., 0., 1.), (0., 0., 2.)))
        with self.assertRaises(SessionError):
            model.add_surface_patches(support, SurfacePatch(), SurfacePatch(((1, 2), (0, 1))))
        self.assertEqual(len(model.objects()), 2)
        self.assertEqual(model.run("Undo").deleted, (left, right))

    def test_trimmed_surface_preserves_support_and_boundary_topology(self):
        model = self.model
        knots = [0, 0, 1, 1]
        support = NurbsSurface(1, 1, knots, knots, [[(0, 0, 0), (0, 1, 0)], [(1, 0, 0), (1, 1, 1)]])
        corners = [(0, 0), (1, 0), (1, 1), (0, 1)]
        outer = [TrimCurve.line(a, b) for a, b in zip(corners, corners[1:] + corners[:1])]
        hole = [
            TrimCurve.line((.25, .25), (.25, .75)),
            TrimCurve(2, [0, 0, 0, 1, 1, 1], [(.25, .75), (.5, .85), (.75, .75)]),
            TrimCurve.line((.75, .75), (.75, .25)),
            TrimCurve.line((.75, .25), (.25, .25)),
        ]
        face = model.add_nurbs_surface(support, name="Trimmed", trim_loops=[outer, hole], opacity=.375)
        path = self.folder / "trim.3dm"
        for reopen in (False, True):
            if reopen:
                model.save(path)
                model.open(path)
            self.assertEqual(model.object(face).kind, "brep")
            self.assertEqual(model.object(face).opacity, .375)
            self.assertEqual(len([item for item in model.components(face) if item.kind == "brep_edge"]), 8)
            self.assertEqual(model.surface_jets(face, (.9, .9))[0].duv, (0., 0., 1.))
        with self.assertRaises(SessionError):
            model.add_surface_patches(support, SurfacePatch(), SurfacePatch(trim_loops=[outer[:3]]))
        with self.assertRaisesRegex(SessionError, "object opacity must be between 0 and 1"):
            model.add_surface_patches(support, SurfacePatch(), SurfacePatch(opacity=1.01))
        self.assertEqual(len(model.objects()), 1)

    def test_sew_explicit_surface_edges_preserves_uv_and_is_atomic(self):
        model = self.model
        knots = [0, 0, 1, 1]
        support = NurbsSurface(1, 1, knots, knots, [[(0, 0, 0), (0, 1, 0)], [(1, 0, 0), (1, 1, 1)]])
        parts = []
        for low, high in ((0., .5), (.5, 1.)):
            corners = [(low, 0.), (high, 0.), (high, 1.), (low, 1.)]
            rim = [TrimCurve.line(a, b) for a, b in zip(corners, corners[1:] + corners[:1])]
            parts.append(model.add_nurbs_surface(support, name="Skin", trim_loops=[rim]))
        boundaries = []
        for identifier in parts:
            seam, = (part for part in model.components(identifier)
                     if part.kind == "brep_edge" and all(point[0] == .5 for point in part.points))
            boundaries.append(seam)
        first, second = boundaries
        reversed = first.points == second.points[::-1]
        with self.assertRaises(SessionError):
            model.sew_brep_edges((first, second, not reversed))
        self.assertEqual([obj.id for obj in model.objects()], parts)
        original = [model.surface_definition(identifier) for identifier in parts]
        joined = model.sew_brep_edges((first, second, reversed))
        self.assertEqual(model.object(joined).name, "Skin")
        archive = self.folder / "sewn.3dm"
        model.save(archive)
        for reopened in (False, True):
            if reopened:
                model.open(archive)
            faces = [part for part in model.components(joined) if part.kind == "brep_face"]
            self.assertEqual(len(faces), 2)
            self.assertEqual(len(set(faces[0].loops[0]) & set(faces[1].loops[0])), 1)
            self.assertEqual([model.surface_definition(joined, face=face.index) for face in faces], original)
            if not reopened:
                self.assertEqual(set(model.run("Undo").added), set(parts))
                self.assertEqual(model.run("Redo").added, (joined,))

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
        wire = model.screenshot(self.folder / "wire.png", view="Top", mode="Wireframe", size=size)
        self.assertNotIn(wire.read_bytes(), captures)
        model.run(ActivateView("Perspective"), "NamedView Save Inspection view")
        named = model.screenshot(self.folder / "named.png", view="Inspection view", size=size)
        self.assertIn(named.read_bytes(), captures)
        camera = Camera(eye=(-9, -18, 11), target=(4, 2, 1), vertical_fov_degrees=31)
        clean = model.screenshot(self.folder / "camera.png", camera=camera, show_edges=False, size=size)
        edges = model.screenshot(self.folder / "camera-edges.png", camera=camera, size=size)
        self.assertNotEqual(clean.read_bytes(), blank)
        self.assertNotEqual(clean.read_bytes(), edges.read_bytes())
        diffuse = model.screenshot(
            self.folder / "diffuse.png", camera=camera, show_edges=False, size=size, lighting="diffuse",
        )
        self.assertNotEqual(diffuse.read_bytes(), blank)
        self.assertNotEqual(diffuse.read_bytes(), clean.read_bytes())
        default_again = model.screenshot(
            self.folder / "default-again.png", camera=camera, show_edges=False, size=size,
        )
        self.assertEqual(clean.read_bytes(), default_again.read_bytes())


if __name__ == "__main__":
    unittest.main()
