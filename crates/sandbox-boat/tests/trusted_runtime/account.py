import json
import os
import pwd
import sys
import uuid

bootstrap = {'__name__': 'fixture_bootstrap'}
exec(compile(sys.argv[1], '<production-bootstrap>', 'exec'), bootstrap)
home, state, workspace, controller = sys.argv[2:]
for account in sorted(pwd.getpwall(), key=lambda entry: entry.pw_uid):
    if account.pw_uid == 0 or account.pw_name == controller:
        continue
    if not account.pw_shell.endswith(('/nologin', '/false')):
        continue
    attempt = uuid.uuid4().hex
    try:
        bootstrap['prepare'](home, state, workspace, controller, account.pw_name, attempt)
    except (KeyError, ValueError):
        continue
    os.rmdir(home + '/.sandbox-boat/' + attempt)
    os.rmdir(state + '/' + attempt)
    print(json.dumps({'name': account.pw_name, 'uid': account.pw_uid}))
    break
else:
    raise SystemExit('no existing restricted account satisfies trusted bootstrap')
