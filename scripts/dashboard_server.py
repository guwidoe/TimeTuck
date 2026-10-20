#!/usr/bin/env python3
"""Serve the local dashboard and report source freshness. Does not run commands."""
import argparse
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
import json
from compat_report import ROOT, verification_digest, completion_digest, native_completion_digest

class Handler(SimpleHTTPRequestHandler):
    def do_GET(self):
        if self.path.split('?')[0] == '/api/freshness':
            payload = json.dumps({'verification_digest': verification_digest(), 'completion_digest': completion_digest(), 'native_completion_digest': native_completion_digest()}).encode()
            self.send_response(200)
            self.send_header('Content-Type', 'application/json')
            self.send_header('Cache-Control', 'no-store')
            self.send_header('Content-Length', str(len(payload)))
            self.end_headers()
            self.wfile.write(payload)
            return
        super().do_GET()

if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--port', type=int, default=8765)
    args = parser.parse_args()
    server = ThreadingHTTPServer(('127.0.0.1', args.port), partial(Handler, directory=str(ROOT / 'dashboard')))
    print(f'TimeTuck dashboard: http://localhost:{args.port}', flush=True)
    try: server.serve_forever()
    except KeyboardInterrupt: server.server_close()
