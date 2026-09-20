"""
Tests for arifFlow Python Client — fail-closed doctrine (2026-08-10).
Verifies default fail-closed behavior and ARIFLOW_FAIL_OPEN override.
"""

import os
import sys
import unittest
from unittest.mock import patch

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


class TestExplanationClassRefusal(unittest.TestCase):
    """CM-1 (2026-09-19): a governance refusal is a VERDICT, not an outage.

    The daemon refuses an execution-class receipt whose declared
    explanation_class is NARRATIVE / UNCLASSIFIED with HTTP 422 and the named
    code F3_EXPLANATION_CLASS_INELIGIBLE. The client must surface that code
    instead of collapsing the answer into 'unreachable' (fail-closed).
    """

    def _http_error(self, code, payload):
        import json as _json
        from email.message import Message
        from io import BytesIO
        from urllib.error import HTTPError

        return HTTPError(
            url="http://127.0.0.1:7073/ingest",
            code=code,
            msg="refused",
            hdrs=Message(),
            fp=BytesIO(_json.dumps(payload).encode()),
        )

    @patch("client.urlopen")
    def test_ingest_refusal_surfaces_named_code(self, mock_urlopen):
        mock_urlopen.side_effect = self._http_error(
            422,
            {
                "status": "refused",
                "refused": True,
                "code": "F3_EXPLANATION_CLASS_INELIGIBLE",
                "violation": "flow-plane",
                "invariant": "F2+F3",
                "verdict_owner": "claim_kernel",
                "actor": "333-AGI",
                "explanation_class": "NARRATIVE",
                "reason": "explanation_class NARRATIVE is not action-eligible",
            },
        )

        client = ArifFlowClient(base_url="http://127.0.0.1:7073")
        result = client.ingest(
            "333-AGI",
            "s-cm1",
            "Execute",
            "Observation",
            1000,
            explanation_class="NARRATIVE",
            explanation_schema="claim_kernel/v1",
        )

        self.assertTrue(result.refused)
        self.assertEqual(result.status, "refused")
        self.assertEqual(result.code, "F3_EXPLANATION_CLASS_INELIGIBLE")
        self.assertEqual(result.explanation_class, "NARRATIVE")
        self.assertIn("not action-eligible", result.reason)

    @patch("client.urlopen")
    def test_check_refusal_surfaces_named_code(self, mock_urlopen):
        mock_urlopen.side_effect = self._http_error(
            403,
            {
                "actor": "333-AGI",
                "allowed": False,
                "action": "Hold",
                "code": "F3_EXPLANATION_CLASS_INELIGIBLE",
                "explanation_class": "UNCLASSIFIED",
                "reason": "UNCLASSIFIED is not action-eligible",
            },
        )

        client = ArifFlowClient(base_url="http://127.0.0.1:7073")
        result = client.check("333-AGI", explanation_class="UNCLASSIFIED")

        self.assertFalse(result.allowed)
        self.assertEqual(result.action, "Hold")
        self.assertEqual(result.code, "F3_EXPLANATION_CLASS_INELIGIBLE")

    @patch("client.urlopen")
    def test_legacy_check_sends_no_explanation_field(self, mock_urlopen):
        """Backward compat: legacy calls transmit only actor_id."""
        import json as _json

        captured = {}

        class _Resp:
            def __enter__(self):
                return self

            def __exit__(self, *a):
                return False

            def read(self):
                return _json.dumps(
                    {"actor": "333-AGI", "allowed": True, "reason": "OK", "action": "Allow"}
                ).encode()

        def fake_urlopen(req, timeout=None):
            captured["body"] = _json.loads(req.data.decode())
            return _Resp()

        mock_urlopen.side_effect = fake_urlopen

        client = ArifFlowClient(base_url="http://127.0.0.1:7073")
        result = client.check("333-AGI")

        self.assertTrue(result.allowed)
        self.assertEqual(captured["body"], {"actor_id": "333-AGI"})


if __name__ == "__main__":
    unittest.main()
