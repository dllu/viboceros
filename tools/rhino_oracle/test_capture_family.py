"""Capture transactions retain source bytes and never replace successful evidence."""
import copy
import gzip
import hashlib
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
from . import capture_family


class CaptureFamilyTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        for folder in ['fixtures', 'observations/source_snapshots']:
            (self.root / folder).mkdir(parents=True)
        self.source = self.root / 'producer.py'
        self.source.write_text('original producer\n')
        (self.root / 'capture_family.py').write_text('capture producer\n')
        self.query = dict(protocol_version=1, iterations=1, operations=[dict(id='one')])
        self.response = dict(engine='rhino', results=[dict(id='one', value=[1., 2.])])
        self.output = self.root / 'observations/example.json'
        self.fixture = self.root / 'fixtures/example.json'
        self.addCleanup(patch.stopall)
        patch.object(capture_family, 'HERE', self.root).start()
        self.client = patch.object(capture_family, 'OracleClient').start()
        self.run = self.client.return_value.run_rhino
        self.run.return_value = copy.deepcopy(self.response)

    def capture(self):
        capture_family.capture('example', self.query, ['producer.py'], 'private-test-scheme', 42)

    def test_frozen_sources_request_and_result_coverage(self):
        self.capture()
        self.client.assert_called_once_with(settings_scheme='private-test-scheme')
        self.run.assert_called_once_with(self.query, timeout=42)
        result = json.loads(self.output.read_text())
        self.assertEqual(result['results'], self.response['results'])
        provenance = result['provenance']
        self.assertEqual(provenance['request_sha256'], hashlib.sha256(self.fixture.read_bytes()).hexdigest())
        self.assertEqual(set(provenance['sources']), {'producer.py', 'capture_family.py'})
        for name, digest in provenance['sources'].items():
            archive = self.root / ('observations/source_snapshots/' + digest + '.gz')
            self.assertEqual(gzip.decompress(archive.read_bytes()), (self.root / name).read_bytes())
        before = self.output.read_bytes()
        with self.assertRaisesRegex(ValueError, 'immutable'):
            self.capture()
        self.assertEqual(self.output.read_bytes(), before)
        self.run.assert_called_once()

    def test_different_fixture_is_preserved_without_launching_rhino(self):
        self.fixture.write_text('existing fixture\n')
        with self.assertRaisesRegex(ValueError, 'fixture differs'):
            self.capture()
        self.assertEqual(self.fixture.read_text(), 'existing fixture\n')
        self.run.assert_not_called()

    def test_producer_mutation_does_not_publish_a_successful_capture(self):
        def run(*args, **kwargs):
            self.source.write_text('changed producer\n')
            return self.response
        self.run.side_effect = run
        with self.assertRaisesRegex(ValueError, 'producer changed'):
            self.capture()
        self.assertFalse(self.output.exists())

    def test_fixture_mutation_does_not_publish_a_successful_capture(self):
        def run(*args, **kwargs):
            self.fixture.write_text('changed fixture\n')
            return self.response
        self.run.side_effect = run
        with self.assertRaisesRegex(ValueError, 'fixture changed'):
            self.capture()
        self.assertFalse(self.output.exists())

    def test_missing_or_reordered_ids_do_not_publish_a_successful_capture(self):
        for rows in [[], [dict(id='two')], [dict(id='one'), dict(id='one')]]:
            self.run.return_value = dict(results=rows)
            with self.subTest(rows=rows), self.assertRaisesRegex(ValueError, 'coverage differs'):
                self.capture()
            self.assertFalse(self.output.exists())

    def test_corrupted_existing_archive_is_preserved_and_blocks_publication(self):
        digest = hashlib.sha256(self.source.read_bytes()).hexdigest()
        archive = self.root / ('observations/source_snapshots/' + digest + '.gz')
        archive.write_bytes(gzip.compress(b'different source', mtime=0))
        before = archive.read_bytes()
        with self.assertRaisesRegex(ValueError, 'source archive differs'):
            self.capture()
        self.assertEqual(archive.read_bytes(), before)
        self.assertFalse(self.output.exists())


    def test_host_client_sources_are_frozen_transitively(self):
        names = capture_family.CLIENT_SOURCE_FILES
        for name in names:
            (self.root / name).write_text('owned helper '+name+'\n')
        capture_family.capture('example', self.query,
                               ['producer.py', 'capture_preflight_components.py'],
                               'private-test-scheme', 42)
        result = json.loads(self.output.read_text())
        self.assertEqual(set(result['provenance']['sources']),
                         {'producer.py', 'capture_family.py', *names})
        for name, digest in result['provenance']['sources'].items():
            archive = self.root / ('observations/source_snapshots/' + digest + '.gz')
            self.assertEqual(gzip.decompress(archive.read_bytes()),
                             (self.root / name).read_bytes())

    def test_host_helper_mutation_cannot_publish_a_capture(self):
        for name in capture_family.CLIENT_SOURCE_FILES:
            (self.root / name).write_text('owned helper '+name+'\n')
        def run(*args, **kwargs):
            (self.root / 'capture_preflight_components.py').write_text('changed\n')
            return self.response
        self.run.side_effect = run
        with self.assertRaisesRegex(ValueError, 'producer changed'):
            capture_family.capture('example', self.query, ['client.py'],
                                   'private-test-scheme', 42)
        self.assertFalse(self.output.exists())


if __name__ == '__main__':
    unittest.main()
