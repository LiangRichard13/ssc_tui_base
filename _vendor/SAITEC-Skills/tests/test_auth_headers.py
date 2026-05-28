import json
import os
import sys
import tempfile
import unittest
from pathlib import Path

import httpx


MCP_SERVER = Path(__file__).resolve().parents[1] / "mcp_server"
sys.path.insert(0, str(MCP_SERVER))

from api_tools.auth_headers import build_auth_headers
from api_tools.http_errors import raise_for_status_with_body


class EnvGuard:
    def __init__(self, *keys: str):
        self._previous = {key: os.environ.get(key) for key in keys}

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc, tb):
        for key, value in self._previous.items():
            if value is None:
                os.environ.pop(key, None)
            else:
                os.environ[key] = value


class AuthHeadersTests(unittest.TestCase):
    def test_reads_api_key_from_saitec_tui_home_auth_json_when_env_missing(self):
        with EnvGuard("SAITEC_API_KEY", "SAITEC_TUI_HOME"):
            with tempfile.TemporaryDirectory() as temp_dir:
                os.environ.pop("SAITEC_API_KEY", None)
                os.environ["SAITEC_TUI_HOME"] = temp_dir
                auth_file = Path(temp_dir) / "auth.json"
                auth_file.write_text(
                    json.dumps({"api_key": "sk-from-auth-json"}),
                    encoding="utf-8",
                )

                self.assertEqual(
                    build_auth_headers(),
                    {"X-API-Key": "sk-from-auth-json"},
                )

    def test_environment_api_key_overrides_auth_json(self):
        with EnvGuard("SAITEC_API_KEY", "SAITEC_TUI_HOME"):
            with tempfile.TemporaryDirectory() as temp_dir:
                os.environ["SAITEC_API_KEY"] = "sk-from-env"
                os.environ["SAITEC_TUI_HOME"] = temp_dir
                auth_file = Path(temp_dir) / "auth.json"
                auth_file.write_text(
                    json.dumps({"api_key": "sk-from-auth-json"}),
                    encoding="utf-8",
                )

                self.assertEqual(build_auth_headers(), {"X-API-Key": "sk-from-env"})


class HttpErrorTests(unittest.TestCase):
    def test_http_status_error_includes_response_body(self):
        request = httpx.Request(
            "POST",
            "http://127.0.0.1:8000/api/v1/skills/text-detect/detect",
        )
        response = httpx.Response(
            403,
            request=request,
            text='{"success":false,"message":"No active subscription found"}',
        )

        with self.assertRaises(httpx.HTTPStatusError) as ctx:
            raise_for_status_with_body(response)

        self.assertIn("403 Forbidden", str(ctx.exception))
        self.assertIn("No active subscription found", str(ctx.exception))


if __name__ == "__main__":
    unittest.main()
