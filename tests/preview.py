"""Isolated visual QA proxy: real rendered pages with delayed/failed images.

python tests/preview.py; open http://127.0.0.1:3211/library?qa=slow
Add &motion=reduce to exercise the shipped reduced-motion CSS without OS changes.
No browser automation and no writes to Jellyfin. Stop this process after review.
"""
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.request import urlopen
from urllib.error import HTTPError
from urllib.parse import urlsplit, parse_qs
from time import sleep


class Preview(BaseHTTPRequestHandler):
    def do_GET(self):
        parsed = urlsplit(self.path)
        params = parse_qs(parsed.query)
        route = self.path
        if parsed.path.startswith('/slow-image/'):
            sleep(4)
            route = route.replace('/slow-image/', '/image/', 1)
        if parsed.path.startswith('/failed-image/'):
            self.send_error(503, 'Intentional image failure for visual QA')
            return
        try:
            response = urlopen('http://127.0.0.1:3210' + route, timeout=30)
        except HTTPError as error:
            response = error
        data = response.read()
        kind = response.headers.get('Content-Type', '')
        if 'text/html' in kind:
            text = data.decode('utf-8')
            if params.get('qa') == ['slow']:
                text = text.replace('/image/', '/slow-image/')
            elif params.get('qa') == ['error']:
                text = text.replace('/image/', '/failed-image/', 1)
            elif params.get('qa') == ['hero-error']:
                text = text.replace('/image/7575761e8891fba63a2e8a305347c573/Backdrop', '/failed-image/7575761e8891fba63a2e8a305347c573/Backdrop')
            if params.get('motion') == ['reduce']:
                text = text.replace('<html lang="es">', '<html lang="es" data-reduce-motion>')
                text = text.replace('/assets/app.css?v=5', '/assets/app.css?v=5&qa-motion=reduce')
            data = text.encode('utf-8')
        elif 'text/css' in kind and params.get('qa-motion') == ['reduce']:
            data = data.replace(b'(prefers-reduced-motion:reduce)', b'(min-width:0px)')
            data = data.replace(b'@view-transition { navigation:auto; }', b'@view-transition { navigation:none; }')
        self.send_response(response.status)
        self.send_header('Content-Type', kind)
        self.send_header('Cache-Control', 'no-store')
        self.send_header('Content-Length', str(len(data)))
        self.end_headers()
        try:
            self.wfile.write(data)
        except (ConnectionResetError, BrokenPipeError, ConnectionAbortedError):
            pass

    def log_message(self, *_):
        pass


if __name__ == '__main__':
    print('Visual QA only: http://127.0.0.1:3211', flush=True)
    ThreadingHTTPServer(('127.0.0.1', 3211), Preview).serve_forever()
