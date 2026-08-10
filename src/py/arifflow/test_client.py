"""
Tests for arifFlow Python Client — fail-closed doctrine (2026-08-10).
Verifies default fail-closed behavior and ARIFLOW_FAIL_OPEN override.
"""

import os
import sys
import unittest
from unittest.mock import patch, MagicMock

# Ensure the module is importable
sys.path.insert(0, os.path.dirname(__file__))
from client import ArifFlowClient, CheckResult


class TestFailClosedDefault(unittest.TestCase):
    """Verify default fail-closed behavior when arifFlow daemon is unreachable."""

    @patch("client.urlopen")
    def test_fail_closed_default(self, mock_urlopen):
        """[OBS] When daemon unreachable, default response is allowed=False (fail-closed)."""
        mock_urlopen.side_effect = ConnectionRefusedError("Connection refused")

        client = ArifFlowClient(base_url="http://127.0.0.1:9999")
        result = client.check("test-actor")

        self.assertIsInstance(result, CheckResult)
        self.assertFalse(result.allowed, "Default must be fail-closed (allowed=False)")
        self.assertEqual(result.action, "Hold", "Default action must be Hold")
        self.assertIn(
            "governance unavailable",
            result.reason,
            "Reason must state governance unavailable",
        )

    @patch("client.urlopen")
    def test_fail_open_override(self, mock_urlopen):
        """[OBS] ARIFLOW_FAIL_OPEN=true overrides to fail-open for emergencies."""
        mock_urlopen.side_effect = ConnectionRefusedError("Connection refused")

        # Set override before import re-evaluation
        os.environ["ARIFLOW_FAIL_OPEN"] = "true"
        try:
            client = ArifFlowClient(base_url="http://127.0.0.1:9999")
            result = client.check("test-actor")

            self.assertIsInstance(result, CheckResult)
            self.assertTrue(
                result.allowed, "Override must allow execution (allowed=True)"
            )
            self.assertEqual(result.action, "Allow", "Override action must be Allow")
            self.assertIn(
                "fail-open override active",
                result.reason,
                "Reason must indicate override is active",
            )
        finally:
            os.environ.pop("ARIFLOW_FAIL_OPEN", None)

    @patch("client.urlopen")
    def test_fail_open_override_via_yes(self, mock_urlopen):
        """[OBS] 'yes' also activates fail-open override."""
        mock_urlopen.side_effect = ConnectionRefusedError("Connection refused")

        os.environ["ARIFLOW_FAIL_OPEN"] = "yes"
        try:
            client = ArifFlowClient(base_url="http://127.0.0.1:9999")
            result = client.check("test-actor")
            self.assertTrue(result.allowed)
        finally:
            os.environ.pop("ARIFLOW_FAIL_OPEN", None)

    @patch("client.urlopen")
    def test_fail_open_override_via_1(self, mock_urlopen):
        """[OBS] '1' also activates fail-open override."""
        mock_urlopen.side_effect = ConnectionRefusedError("Connection refused")

        os.environ["ARIFLOW_FAIL_OPEN"] = "1"
        try:
            client = ArifFlowClient(base_url="http://127.0.0.1:9999")
            result = client.check("test-actor")
            self.assertTrue(result.allowed)
        finally:
            os.environ.pop("ARIFLOW_FAIL_OPEN", None)

    @patch("client.urlopen")
    def test_fail_open_remains_closed_on_false(self, mock_urlopen):
        """[OBS] ARIFLOW_FAIL_OPEN=false does NOT activate override (stays closed)."""
        mock_urlopen.side_effect = ConnectionRefusedError("Connection refused")

        os.environ["ARIFLOW_FAIL_OPEN"] = "false"
        try:
            client = ArifFlowClient(base_url="http://127.0.0.1:9999")
            result = client.check("test-actor")
            self.assertFalse(result.allowed, "'false' must not trigger override")
        finally:
            os.environ.pop("ARIFLOW_FAIL_OPEN", None)


if __name__ == "__main__":
    unittest.main()
