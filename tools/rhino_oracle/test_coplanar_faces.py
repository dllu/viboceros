"""Public SDK routing/ownership mocks, without claims of licensed parity."""
import ast
import copy
from pathlib import Path
from types import SimpleNamespace as NS
from unittest import TestCase, mock
from .coplanar_faces_probe import request, validate, run

class Faces(list):
    @property
    def Count(self): return len(self)

class CoplanarTests(TestCase):
    def host(self, solid=False):
        class Brep: pass
        source = Brep()
        source.IsSolid = solid; source.IsValid = True
        source.Faces = Faces([NS(Loops=NS(Count=1)) for _ in range(2)])
        source.Dispose = mock.Mock()
        output = mock.Mock(IsValid=True, IsSolid=solid)
        output.Faces = Faces([NS(Loops=NS(Count=1))])
        output.MergeCoplanarFaces.return_value = True
        output.GetBoundingBox.return_value = NS(Min=[0,0,0], Max=[4,3,0])
        source.DuplicateBrep = mock.Mock(return_value=output)
        area = mock.Mock(Area=12.); volume = mock.Mock(Volume=24.)
        G = NS(Brep=Brep, AreaMassProperties=NS(Compute=mock.Mock(return_value=area)),
               VolumeMassProperties=NS(Compute=mock.Mock(return_value=volume)))
        host = {'Rhino':NS(Geometry=G), '_object_source':lambda *args:source,
                '_measure':lambda n,f:([f() for _ in range(n)][-1],0), '_xyz':lambda p:p}
        return host, source, output, area, volume

    def test_strict_recipes(self):
        for op in request()['operations']: validate(op)
        for update in [dict(extra=1),dict(source=dict(type='mesh')),dict(op='merge_face')]:
            op=copy.deepcopy(request()['operations'][0]);op.update(update)
            with self.subTest(update=update), self.assertRaises(ValueError): validate(op)
        for field,value in [('vertices',[[True,0,0]]*4),('vertices',[[0,0,float('nan')]]*4),
                            ('faces',[[0,1,99]]),('faces',[[0,1,True]]),('faces',[[0,0,1]])]:
            op=copy.deepcopy(request()['operations'][0]);op['source'][field]=value
            with self.subTest(field=field), self.assertRaises(ValueError): validate(op)

    def test_owned_sdk_call_mass_and_noop(self):
        for solid in (False,True):
            host, source, output, area, volume=self.host(solid)
            value,_=run(request()['operations'][0],host,1,request()['tolerance'])
            output.MergeCoplanarFaces.assert_called_once_with(1e-9,1e-8)
            self.assertEqual(value['removed_faces'],1)
            self.assertEqual(value['area'],12.)
            self.assertEqual(value['volume'],24. if solid else None)
            for obj in (source,output,area):obj.Dispose.assert_called_once()
            if solid: volume.Dispose.assert_called_once()
        host, source, output, _, _=self.host()
        output.Faces=source.Faces;output.MergeCoplanarFaces.return_value=False
        value,_=run(request()['operations'][0],host,2,request()['tolerance'])
        self.assertEqual(value['removed_faces'],0)
        self.assertEqual(source.DuplicateBrep.call_count,2)
        self.assertEqual(output.Dispose.call_count,2)
        source.Dispose.assert_called_once()

    def test_failure_cleanup(self):
        for stage in ('copy','native','invalid','solid','area','mass','false_change','bounds'):
            host,source,output,area,volume=self.host(solid=True)
            if stage=='copy':source.DuplicateBrep.return_value=None
            elif stage=='native':output.MergeCoplanarFaces.side_effect=ValueError('failed')
            elif stage=='invalid':output.IsValid=False
            elif stage=='solid':output.IsSolid=False
            elif stage=='area':host['Rhino'].Geometry.AreaMassProperties.Compute.return_value=None
            elif stage=='mass':host['Rhino'].Geometry.VolumeMassProperties.Compute.return_value=None
            elif stage=='bounds':output.GetBoundingBox.return_value=NS(Min=[float('nan'),0,0],Max=[4,3,0])
            else:output.MergeCoplanarFaces.return_value=False
            with self.subTest(stage=stage),self.assertRaises(ValueError):
                run(request()['operations'][0],host,1,request()['tolerance'])
            source.Dispose.assert_called_once()
            if stage!='copy':output.Dispose.assert_called_once()
            if stage=='mass':area.Dispose.assert_called_once()

    def test_mesh_brep_input_disposes_mesh_on_success_and_failure(self):
        tree=ast.parse(Path(__file__).with_name('rhino_worker.py').read_text())
        node=next(n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name=='_object_source')
        mesh=mock.Mock();construct=mock.Mock(return_value=mock.sentinel.brep)
        scope={'Rhino':NS(Geometry=NS(Brep=NS(CreateFromMesh=construct))), '_polygon_mesh':lambda *args:mesh}
        exec(compile(ast.Module(body=[node],type_ignores=[]),'<object_source>','exec'),scope)
        source=request()['operations'][0]['source']
        self.assertIs(scope['_object_source'](source,{}),mock.sentinel.brep)
        construct.assert_called_once_with(mesh,True);mesh.Dispose.assert_called_once()
        mesh.reset_mock();construct.return_value=None
        with self.assertRaises(ValueError):scope['_object_source'](source,{})
        mesh.Dispose.assert_called_once()
