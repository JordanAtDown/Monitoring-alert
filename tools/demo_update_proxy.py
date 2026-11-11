#!/usr/bin/env python3
"""DEMO ONLY: forwards everything to the real dashboard, except /api/update, which pretends that a
newer version exists, so that the update button and its window can be seen.

Usage: demo_update_proxy.py <dashboard-port> <listen-port>
"""
import http.server, json, sys, urllib.error, urllib.request

UPSTREAM, LISTEN = int(sys.argv[1]), int(sys.argv[2])
FAKE = {
    "current": "2.0.0", "enabled": True, "available": True, "checked_at": "2026-11-11T15:00:00", "error": None,
    "latest": {
        "version": "9.9.9",
        "page_url": "https://github.com/JordanAtDown/Monitoring-alert/releases",
        "download_url": None,
        "notes": "(démonstration : cette version n'existe pas)\n- Exemple de nouveautés",
        "published_at": "2026-11-20T10:00:00Z",
    },
}


class Proxy(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path.startswith("/api/update"):
            body, ctype, code = json.dumps(FAKE).encode(), "application/json", 200
        else:
            request = urllib.request.Request(f"http://127.0.0.1:{UPSTREAM}{self.path}", headers={"Host": f"127.0.0.1:{UPSTREAM}"})
            try:
                r = urllib.request.urlopen(request)
                body, ctype, code = r.read(), r.headers.get("Content-Type"), r.status
            except urllib.error.HTTPError as e:
                body, ctype, code = e.read(), e.headers.get("Content-Type"), e.code
        self.send_response(code)
        self.send_header("Content-Type", ctype or "text/plain")
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, *args):
        pass


http.server.ThreadingHTTPServer(("127.0.0.1", LISTEN), Proxy).serve_forever()
