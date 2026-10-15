"""SDK ownership and observation tests; these mocks are not licensed parity evidence."""
import ast
import copy
from pathlib import Path
from types import SimpleNamespace as NS
from unittest import TestCase, mock
from .cull_degenerate_probe import request, validate, run


class Faces(list):
    @property
    def Count(self): return len(self)


class CleanupTests(TestCase):
    def test_actual_worker_routes_to_the_staged_sdk_companion(self):
        tree=ast.parse(Path(__file__).with_name('rhino_worker.py').read_text())
        node=next(n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name=='_execute')
        scope={}
        exec(compile(ast.Module(body=[node],type_ignores=[]),'<actual worker route>','exec'),scope)
        execute=mock.Mock(return_value=mock.sentinel.observation)
        operation=request()['operations'][0];tolerance=request()['tolerance']
        with mock.patch.dict('sys.modules',{'cull_degenerate_probe':NS(run=execute)}):
            self.assertIs(scope['_execute'](operation,2,tolerance),mock.sentinel.observation)
        execute.assert_called_once_with(operation,scope,2,tolerance)

    def host(self):
        source = mock.Mock(IsValid=False)
        source.Faces.Count = 1; source.Vertices.Count = 4
        output = mock.Mock()
        output.Faces = Faces([NS(IsQuad=True)])
        output.Vertices.Count = 4
        def repair():
            output.Faces[0].IsQuad = False
            return 0  # A repair need not remove a face.
        output.Faces.CullDegenerateFaces = mock.Mock(side_effect=repair)
        output.Vertices.CullUnused.side_effect = lambda: setattr(output.Vertices, 'Count', 3)
        source.DuplicateMesh.return_value = output
        mass = mock.Mock(Area=0.5)
        host = {'_object_source':mock.Mock(return_value=source),
                '_measure':lambda n,f:([f() for _ in range(n)][-1],0),
                '_polygon_mesh_value':mock.Mock(return_value={'faces':[[0,1,2]],'vertices':[[0,0,0],[1,0,0],[0,1,0]]}),
                'Rhino':NS(Geometry=NS(AreaMassProperties=NS(Compute=mock.Mock(return_value=mass))))}
        return host, source, output, mass

    def test_recipes_strict_before_any_native_access(self):
        for op in request()['operations']: validate(op)
        original = request()['operations'][0]
        for updates in [dict(extra=True),dict(source=None),dict(op='cull'),dict(id=True),dict(source=dict(type='mesh'))]:
            op=copy.deepcopy(original);op.update(updates)
            with self.subTest(updates=updates),self.assertRaises(ValueError):validate(op)
        for field,value in [('vertices',[[True,0,0]]*3),('vertices',[[float('nan'),0,0]]*3),
                            ('faces',[[0,1,3]]),('faces',[[0,0,1]]),('faces',[])]:
            op=copy.deepcopy(original);op['source'][field]=value
            with self.subTest(field=field),self.assertRaises(ValueError):validate(op)

    def test_invalid_source_is_copied_and_repair_is_observed_despite_zero_return(self):
        host,source,output,mass=self.host()
        value,_=run(request()['operations'][3],host,2,request()['tolerance'])
        self.assertEqual((value['removed_faces'],value['removed_vertices'],value['quad_count'],value['triangle_count']),(0,1,0,1))
        self.assertEqual(value['area'],0.5)
        self.assertEqual(output.Faces.CullDegenerateFaces.call_count,2)
        self.assertEqual(output.Vertices.CullUnused.call_count,2)
        self.assertEqual(output.Dispose.call_count,2)
        self.assertEqual(mass.Dispose.call_count,2)
        source.Dispose.assert_called_once()

    def test_noop_empty_and_preexisting_orphans_use_actual_post_counts(self):
        for empty,orphan in [(False,False),(False,True),(True,False)]:
            host,source,output,mass=self.host()
            output.Faces.CullDegenerateFaces.side_effect=None
            output.Vertices.CullUnused.side_effect=None
            output.Faces[0].IsQuad=False
            source.Vertices.Count=4 if orphan else 3
            output.Vertices.Count=0 if empty else 3
            if empty:
                output.Faces.clear()
                host['_polygon_mesh_value'].return_value={'faces':[],'vertices':[]}
            value,_=run(request()['operations'][0],host,1,request()['tolerance'])
            self.assertEqual(value['empty'],empty)
            self.assertEqual(value['removed_vertices'],3 if empty else int(orphan))
            self.assertEqual(value['removed_faces'],int(empty))
            if empty:
                self.assertEqual(value['area'],0.)
                mass.Dispose.assert_not_called()
                host['Rhino'].Geometry.AreaMassProperties.Compute.assert_not_called()

    def test_all_failure_paths_dispose_owned_copies_and_masses(self):
        for stage in ('source','copy','alias','cull','vertices','area','observe'):
            host,source,output,mass=self.host()
            if stage=='source':host['_object_source'].return_value=None
            elif stage=='copy':source.DuplicateMesh.return_value=None
            elif stage=='alias':source.DuplicateMesh.return_value=source
            elif stage=='cull':output.Faces.CullDegenerateFaces.side_effect=ValueError('cull failed')
            elif stage=='vertices':output.Vertices.CullUnused.side_effect=ValueError('vertices failed')
            elif stage=='area':host['Rhino'].Geometry.AreaMassProperties.Compute.return_value=None
            else:host['_polygon_mesh_value'].side_effect=ValueError('observation failed')
            with self.subTest(stage=stage),self.assertRaises(ValueError):
                run(request()['operations'][0],host,1,request()['tolerance'])
            if stage!='source':source.Dispose.assert_called_once()
            if stage in ('cull','vertices','area','observe'):output.Dispose.assert_called_once()
            else:output.Dispose.assert_not_called()
            if stage=='observe':mass.Dispose.assert_called_once()

    def test_worker_raw_observation_keeps_double_coordinate_distinctions(self):
        tree=ast.parse(Path(__file__).with_name('rhino_worker.py').read_text())
        node=next(n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name=='_polygon_mesh_value')
        vertices=mock.Mock();vertices.Count=3
        points=[[1e6,0,0],[1e6+1e-4,0,0],[1e6,1e-4,0]]
        vertices.Point3dAt.side_effect=lambda i:points[i]
        scope={'_xyz':lambda p:p}
        exec(compile(ast.Module(body=[node],type_ignores=[]),'<raw mesh observer>','exec'),scope)
        value=scope['_polygon_mesh_value'](NS(Vertices=vertices,Faces=Faces([NS(A=0,B=1,C=2,IsQuad=False)])))
        self.assertEqual(value['vertices'],points)
        self.assertNotEqual(value['vertices'][0],value['vertices'][1])
