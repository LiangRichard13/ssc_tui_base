"""Local mock of SAITEC backend endpoints for reproducing the TUI-update banner bug.

Responds to:
  GET /api/v1/tui/check-update?current_version=...  -> is_new=true (banner should appear)
  GET /api/v1/tui/download[?version=...]            -> stub download (optional)

Run: python scripts/mock_saitec_update_server.py [port]   (default 8000)
Then launch the TUI with CORE_API_BASE=http://127.0.0.1:8000
"""

import http.server
import json
import sys

PORT = int(sys.argv[1]) if len(sys.argv) > 1 else 8000

# A version we are sure is "newer" than any selfdev build (1.x).
PAYLOAD = {
    "latest_version": "99.0.0",
    "is_new": True,
    "filename": "v99.0.0.exe",
    "size_bytes": 123456789,
    "release_notes": "mock release notes for local reproduction",
    "download_url": f"http://127.0.0.1:{PORT}/api/v1/tui/download?version=99.0.0",
}


class Handler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        path = self.path.split("?")[0]
        print(f"[mock] GET {self.path}", flush=True)
        if path.endswith("/api/v1/tui/check-update"):
            body = json.dumps({"success": True, "message": "ok", "data": PAYLOAD}).encode()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
        elif path.endswith("/api/v1/tui/download"):
            # Minimal stub so a download attempt doesn't 404.
            body = b"mock exe payload"
            self.send_response(200)
            self.send_header("Content-Type", "application/octet-stream")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
        else:
            body = json.dumps({"success": False, "message": f"not found: {path}"}).encode()
            self.send_response(404)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

    def log_message(self, fmt, *args):
        pass


if __name__ == "__main__":
    print(f"mock SAITEC update server on http://127.0.0.1:{PORT}", flush=True)
    http.server.HTTPServer(("127.0.0.1", PORT), Handler).serve_forever()
