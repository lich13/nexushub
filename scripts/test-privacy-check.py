#!/usr/bin/env python3
"""Synthetic regression cases; no private deployment values are stored here."""
import importlib.util
from pathlib import Path
import sys
import unittest

sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("privacy", Path(__file__).with_name("privacy-check.py"))
privacy = importlib.util.module_from_spec(spec)
spec.loader.exec_module(privacy)


class PrivacyChecks(unittest.TestCase):
    def test_reserved_examples_and_public_repository_are_allowed(self):
        self.assertEqual(privacy.problems(b"https://panel.example.com/nexushub/ 192.0.2.10 /Users/example/project author@users.noreply.github.com https://github.com/lich13/nexushub", []), [])

    def test_unknown_endpoint_private_home_and_secret_are_rejected(self):
        data = b"https://" + b"personal" + b".invalid-host/ /Users/" + b"synthetic-person/project ghp_" + b"a" * 36
        rules = privacy.problems(data, [])
        self.assertIn("unreviewed_endpoint", rules)
        self.assertIn("private_home", rules)
        self.assertIn("credential", rules)

    def test_native_payload_checks_private_values_without_string_table_false_positives(self):
        data = b"\x7fELF\0http://concatenatedliteral\0fixture-private-value\0/Users/" + b"synthetic-person/source"
        self.assertEqual(privacy.problems(data, ["fixture-private-value"]), ["private_home", "private_value"])

    def test_generated_frontend_skips_lexical_url_false_positives(self):
        bundle = b'"ht' + b'tps://www.css";const source="https://service.example.invalid";'
        self.assertIn("unreviewed_endpoint", privacy.problems(bundle, []))
        self.assertEqual(privacy.problems(bundle, [], scan_endpoint_literals=False), [])


if __name__ == "__main__":
    unittest.main()
