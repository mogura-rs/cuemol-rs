#!/usr/bin/env python3
"""Check native credential storage on a disposable CI runner with a loopback API."""
import argparse
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import secrets
import subprocess
import tempfile
import threading


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', required=True, type=Path)
    parser.add_argument('--out', required=True, type=Path)
    args = parser.parse_args()
    if (os.environ.get('GITHUB_ACTIONS') != 'true'
            or os.environ.get('CUEMOL_CREDENTIAL_TEST_RUNNER') != 'github-hosted'):
        parser.error('Run only on a disposable GitHub-hosted runner')
    binary = args.binary.resolve()
    providers = {'openai': 'OPENAI_API_KEY', 'anthropic': 'ANTHROPIC_API_KEY',
                 'google': 'GEMINI_API_KEY'}
    expected = {}
    requests = []

    class Handler(BaseHTTPRequestHandler):
        def log_message(self, *_):
            pass

        def do_GET(self):
            provider = self.path.split('/')[1]
            token = expected.get(provider)
            header = {'openai': 'Authorization', 'anthropic': 'x-api-key',
                      'google': 'x-goog-api-key'}.get(provider)
            wanted = ('Bearer ' if provider == 'openai' else '') + (token or '')
            valid = token is not None and self.headers.get(header, '') == wanted
            requests.append((provider, valid))
            body = {'models': [{'name': 'models/validation-model',
                                'supportedGenerationMethods': ['generateContent']}]}
            if provider != 'google':
                body = {'data': [{'id': 'validation-model'}]}
            payload = json.dumps(body if valid else {'error': 'Unexpected credential'}).encode()
            self.send_response(200 if valid else 403)
            self.send_header('Content-Type', 'application/json')
            self.send_header('Content-Length', str(len(payload)))
            self.end_headers()
            self.wfile.write(payload)

    server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    results = {}
    try:
        with tempfile.TemporaryDirectory(prefix='cuemol-credential-check-') as directory:
            env = {name: value for name, value in os.environ.items()
                   if not name.startswith('CUEMOL_') and name not in providers.values()
                   and name.upper() not in ('HTTP_PROXY', 'HTTPS_PROXY', 'ALL_PROXY')}
            env['CUEMOL_CONFIG_DIR'] = directory
            endpoint = f'http://127.0.0.1:{server.server_port}'

            def command(arguments, value=None, extra_env=None):
                return subprocess.run([str(binary), 'agent', *arguments], input=value,
                                      env=dict(env, **(extra_env or {})), cwd=directory,
                                      text=True, capture_output=True, timeout=45)

            def models(provider, **kwargs):
                return command(['models', '--model', f'{provider}:validation-model',
                                '--endpoint', f'{endpoint}/{provider}'], **kwargs)

            def missing(provider):
                before = len(requests)
                result = models(provider)
                if result.returncode == 0 or f'No {provider} API key is available.' not in result.stderr:
                    raise RuntimeError(f'{provider}: refusing to replace an existing credential')
                if len(requests) != before:
                    raise RuntimeError(f'{provider}: unexpected request without a test credential')

            def check_models(provider, **kwargs):
                before = len(requests)
                result = models(provider, **kwargs)
                if result.returncode or result.stdout.strip() != f'{provider}:validation-model':
                    raise RuntimeError(f'{provider}: credential lookup did not return the test model')
                if requests[before:] != [(provider, True)]:
                    raise RuntimeError(f'{provider}: unexpected credential request')

            for provider in providers:
                missing(provider)
            for provider, variable in providers.items():
                saved = False
                try:
                    for _ in range(2):
                        token = 'cuemol-validation-' + secrets.token_urlsafe(24)
                        result = command(['key', 'set', provider], value=token + '\n')
                        if result.returncode:
                            raise RuntimeError(f'{provider}: credential save failed')
                        saved = True
                        expected[provider] = token
                        check_models(provider)
                finally:
                    if saved:
                        check_models(provider)
                        result = command(['key', 'delete', provider])
                        if result.returncode:
                            raise RuntimeError(f'{provider}: credential cleanup failed')
                missing(provider)
                check_models(provider, extra_env={variable: token})
                results[provider] = {'save': True, 'load': True, 'update': True,
                                     'delete': True, 'environmentFallback': True}
    finally:
        server.shutdown()
        server.server_close()
        thread.join(timeout=5)
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps({'ok': True, 'providers': results,
                                    'authenticatedLoopbackRequests': len(requests)}, indent=2) + '\n')
    print('Native credential save, load, update, delete and environment fallback passed for three providers')


if __name__ == '__main__':
    main()
