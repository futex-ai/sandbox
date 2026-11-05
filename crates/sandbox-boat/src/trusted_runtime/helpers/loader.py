import base64
import hashlib
import json
import os
import pwd
import stat
import sys

FLAGS = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC


def open_path(path):
    fd = os.open('/', FLAGS)
    try:
        parts = path.split('/')[1:]
        if not path.startswith('/') or any(p in ('', '.', '..') for p in parts):
            raise ValueError()
        for part in parts:
            child = os.open(part, FLAGS, dir_fd=fd)
            os.close(fd)
            fd = child
        return fd
    except BaseException:
        os.close(fd)
        raise


def load(home, state, controller, attempt, digest, action):
    if os.geteuid() != 0 or len(attempt) != 32 or any(c not in '0123456789abcdef' for c in attempt):
        raise ValueError()
    if len(digest) != 64 or any(c not in '0123456789abcdef' for c in digest):
        raise ValueError()
    account = pwd.getpwnam(controller)
    stage = open_path(home + '/.sandbox-boat/' + attempt)
    state_fd = open_path(state + '/' + attempt)
    for fd, uid in [(stage, account.pw_uid), (state_fd, 0)]:
        info = os.fstat(fd)
        if info.st_uid != uid or stat.S_IMODE(info.st_mode) != 0o700:
            raise ValueError()
    source = os.open('bundle.json', os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC,
                     dir_fd=stage)
    info = os.fstat(source)
    if not stat.S_ISREG(info.st_mode) or info.st_uid != account.pw_uid or info.st_size > 2 * 1024 * 1024:
        raise ValueError()
    content = bytearray()
    while len(content) <= 2 * 1024 * 1024:
        chunk = os.read(source, 65536)
        if not chunk:
            break
        content.extend(chunk)
    os.close(source)
    if hashlib.sha256(content).hexdigest() != digest:
        raise ValueError()
    bundle = json.loads(content)
    if bundle['version'] != 1 or bundle['attempt'] != attempt:
        raise ValueError()
    os.chdir('/')
    namespace = {'__name__': 'boat_trusted_runtime', 'bundle': bundle, 'stage': stage,
                 'state_fd': state_fd, 'state_root': state, 'home_root': home,
                 'controller_uid': account.pw_uid, 'controller_gid': account.pw_gid,
                 'action': action, 'attempt': attempt}
    sources = bundle['sources']
    for name in ['common', 'process', 'files']:
        exec(compile(base64.b64decode(sources[name], validate=True),
                     '<boat-trusted-' + name + '>', 'exec'), namespace)
    namespace['dispatch']()


try:
    load(*sys.argv[1:])
except SystemExit:
    raise
except BaseException:
    raise SystemExit(49)
