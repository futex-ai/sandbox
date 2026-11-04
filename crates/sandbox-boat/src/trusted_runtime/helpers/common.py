import base64
import ctypes
import contextlib
import fcntl
import errno
import hashlib
import json
import os
import pwd
import selectors
import shutil
import signal
import stat
import subprocess
import sys
import time

DIRECTORY_FLAGS = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC
FILE_FLAGS = os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC
CHUNK = 4 * 1024 * 1024
TRANSFER_LIMIT = 256 * 1024 * 1024
manifest = bundle['manifest']
workload = pwd.getpwnam(bundle['workload_user'])
if workload.pw_uid in (0, controller_uid) or workload.pw_gid in (0, controller_gid):
    raise ValueError()


def read_file_at(directory, name, maximum):
    fd = os.open(name, FILE_FLAGS, dir_fd=directory)
    try:
        info = os.fstat(fd)
        if not stat.S_ISREG(info.st_mode) or info.st_size > maximum:
            raise ValueError()
        content = bytearray()
        while len(content) <= maximum:
            chunk = os.read(fd, min(65536, maximum + 1 - len(content)))
            if not chunk:
                return bytes(content)
            content.extend(chunk)
        raise ValueError()
    finally:
        os.close(fd)


def write_at(directory, name, value, uid=0, gid=0):
    fd = os.open(name, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_CLOEXEC,
                 0o600, dir_fd=directory)
    try:
        view = memoryview(value)
        while view:
            view = view[os.write(fd, view):]
        os.fchown(fd, uid, gid)
        os.fsync(fd)
    finally:
        os.close(fd)
    os.fsync(directory)


def emit(kind, **fields):
    value = {'version': 1, 'attempt': attempt, 'kind': kind, **fields}
    data = json.dumps(value, separators=(',', ':'), ensure_ascii=True).encode('ascii') + b'\n'
    view = memoryview(data)
    while view:
        view = view[os.write(1, view):]


def marker(name):
    try:
        value = read_file_at(state_fd, name, 64)
    except FileNotFoundError:
        return False
    if value != attempt.encode('ascii'):
        raise ValueError()
    return True


def publish(name):
    try:
        write_at(state_fd, name, attempt.encode('ascii'))
    except FileExistsError:
        if not marker(name):
            raise ValueError()


def wait_marker(name, until):
    while not marker(name):
        if marker('cancel') or time.monotonic() >= until:
            raise TimeoutError()
        time.sleep(0.01)


def child_identity(pid):
    with open('/proc/' + str(pid) + '/stat', 'rb') as source:
        fields = source.read(65536).rsplit(b') ', 1)[1].split()
    return fields[19].decode('ascii')


@contextlib.contextmanager
def child_lock():
    fd = os.open('child-lock', os.O_RDWR | os.O_CREAT | os.O_NOFOLLOW | os.O_CLOEXEC,
                 0o600, dir_fd=state_fd)
    try:
        info = os.fstat(fd)
        if not stat.S_ISREG(info.st_mode) or info.st_uid != 0:
            raise ValueError()
        fcntl.flock(fd, fcntl.LOCK_EX)
        yield
    finally:
        os.close(fd)


def terminate_group():
    with child_lock():
        try:
            identity = json.loads(read_file_at(state_fd, 'child.json', 1024))
        except FileNotFoundError:
            publish('cancel')
            return
        if marker('ended'):
            return
        pid = identity['pid']
        try:
            current = child_identity(pid)
            supervisor = child_identity(identity['supervisor'])
        except FileNotFoundError:
            return
        if (identity['attempt'] != attempt or pid <= 0 or current != identity['start']
                or supervisor != identity['supervisor_start']):
            raise ValueError()
        with open('/proc/' + str(pid) + '/stat', 'rb') as source:
            parent = int(source.read(65536).rsplit(b') ', 1)[1].split()[1])
        if parent != identity['supervisor']:
            raise ValueError()
        pidfd = os.pidfd_open(pid, 0)
        try:
            if child_identity(pid) != identity['start']:
                raise ValueError()
            os.killpg(pid, signal.SIGKILL)
        finally:
            os.close(pidfd)


def clean_attempt():
    for fd in [stage, state_fd]:
        for name in os.listdir(fd):
            info = os.stat(name, dir_fd=fd, follow_symlinks=False)
            if not stat.S_ISREG(info.st_mode):
                raise ValueError()
            os.unlink(name, dir_fd=fd)
        os.fsync(fd)
    os.rmdir(home_root + '/.sandbox-boat/' + attempt)
    os.rmdir(state_root + '/' + attempt)


def handoff():
    os.setgroups([])
    os.setgid(workload.pw_gid)
    os.setuid(workload.pw_uid)
    libc = ctypes.CDLL(None, use_errno=True)
    if libc.prctl(38, 1, 0, 0, 0) != 0:
        raise OSError(ctypes.get_errno())
    os.umask(0o022)
