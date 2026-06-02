"""Small HTTP server fixture for automation tests."""

from __future__ import annotations

from http.server import BaseHTTPRequestHandler, HTTPServer


def health_payload() -> dict[str, str]:
    """Return the fixture health payload."""
    return {"status": "ok"}


class HealthHandler(BaseHTTPRequestHandler):
    """Serve the fixture health endpoint."""

    def do_GET(self) -> None:
        """Handle GET requests for the health endpoint."""
        if self.path != "/health":
            self.send_response(404)
            self.end_headers()
            return
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.end_headers()
        self.wfile.write(b'{"status":"ok"}')


def run(port: int) -> None:
    """Run the fixture HTTP server on the requested port."""
    server = HTTPServer(("127.0.0.1", port), HealthHandler)
    server.serve_forever()
