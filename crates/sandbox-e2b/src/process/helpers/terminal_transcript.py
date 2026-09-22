import ctypes
import json
import os
import resource
import shlex
import signal
import stat
import sys
import time

FAILED = 1
LOG_DESCRIPTOR = 3
PR_SET_PDEATHSIG = 1
READ_SIZE = 65536
STOP_GRACE_SECONDS = 2
DIRECTORY_FLAGS = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC
SHELL_HELPER = r'''import os
import pwd
import resource
import sys

FAILED = 1


def fail():
    os._exit(FAILED)


try:
    username = sys.argv[1]
    account = pwd.getpwnam(username)
    if account.pw_uid == 0:
        fail()
    _, hard_limit = resource.getrlimit(resource.RLIMIT_FSIZE)
    resource.setrlimit(resource.RLIMIT_FSIZE, (hard_limit, hard_limit))
    maximum = os.sysconf('SC_OPEN_MAX')
    if maximum < 0:
        maximum = 1048576
    os.closerange(3, maximum)
    os.initgroups(username, account.pw_gid)
    os.setgid(account.pw_gid)
    os.setuid(account.pw_uid)
    if os.geteuid() != account.pw_uid or os.getegid() != account.pw_gid:
        fail()
    os.environ['HOME'] = account.pw_dir
    os.environ['USER'] = username
    os.environ['LOGNAME'] = username
    os.environ['SHELL'] = '/bin/bash'
    os.environ.pop('SUDO_COMMAND', None)
    os.environ.pop('SUDO_GID', None)
    os.environ.pop('SUDO_UID', None)
    os.environ.pop('SUDO_USER', None)
    os.umask(0o022)
    os.execv('/bin/bash', ['bash', '-il'])
except (IndexError, KeyError, OSError, ValueError):
    fail()
'''


def fail():
    os._exit(FAILED)


def descriptor_limit():
    maximum = os.sysconf('SC_OPEN_MAX')
    return maximum if maximum >= 0 else 1048576


def close_except(kept):
    start = 0
    for descriptor in sorted(kept):
        if descriptor > start:
            os.closerange(start, descriptor)
        start = descriptor + 1
    os.closerange(start, descriptor_limit())


def write_all(descriptor, data):
    remaining = memoryview(data)
    while remaining:
        written = os.write(descriptor, remaining)
        if written <= 0:
            fail()
        remaining = remaining[written:]


def record_transcript(reader, writer, limit):
    remaining = limit
    while True:
        chunk = os.read(reader, READ_SIZE)
        if not chunk:
            return
        if remaining > 0:
            bounded = chunk[:remaining]
            write_all(writer, bounded)
            remaining -= len(bounded)


def arm_parent_death(expected_parent):
    libc = ctypes.CDLL(None, use_errno=True)
    if libc.prctl(PR_SET_PDEATHSIG, signal.SIGTERM, 0, 0, 0) != 0:
        fail()
    if os.getppid() != expected_parent:
        fail()


def exit_with_status(status):
    code = os.waitstatus_to_exitcode(status)
    if code < 0:
        code = 128 - code
    os._exit(min(code, 255))


def stop_recorder(pid):
    try:
        os.kill(pid, signal.SIGTERM)
    except ProcessLookupError:
        return
    deadline = time.monotonic() + STOP_GRACE_SECONDS
    while time.monotonic() < deadline:
        try:
            waited, _ = os.waitpid(pid, os.WNOHANG)
        except ChildProcessError:
            return
        if waited == pid:
            return
        time.sleep(0.01)
    try:
        os.kill(pid, signal.SIGKILL)
    except ProcessLookupError:
        pass
    try:
        os.waitpid(pid, 0)
    except ChildProcessError:
        pass


def open_parent(path):
    if not path.startswith('/') or path == '/' or os.path.normpath(path) != path:
        fail()
    parts = path.split('/')[1:]
    if not parts or any(part in ('', '.', '..') for part in parts):
        fail()
    directory = os.open('/', DIRECTORY_FLAGS)
    try:
        for part in parts[:-1]:
            child = os.open(part, DIRECTORY_FLAGS, dir_fd=directory)
            os.close(directory)
            directory = child
        return directory, parts[-1]
    except Exception:
        os.close(directory)
        raise


directory = None
identity = None
identity_temporary_name = None
transcript = None
pipe_reader = None
pipe_writer = None
recorder_pid = None
try:
    path = sys.argv[1]
    identity_path = sys.argv[2]
    limit = int(sys.argv[3])
    workload_user = sys.argv[4]
    terminal_id = sys.argv[5]
    operation_id = sys.argv[6]
    tag = sys.argv[7]
    if limit < 0:
        fail()
    directory, name = open_parent(path)
    identity_directory, identity_name = open_parent(identity_path)
    identity_parent = os.fstat(identity_directory)
    transcript_parent = os.fstat(directory)
    os.close(identity_directory)
    if (identity_parent.st_dev, identity_parent.st_ino) != (
        transcript_parent.st_dev,
        transcript_parent.st_ino,
    ):
        fail()
    if transcript_parent.st_uid != os.geteuid():
        fail()
    if stat.S_IMODE(transcript_parent.st_mode) & 0o077:
        fail()
    try:
        os.stat(identity_name, dir_fd=directory, follow_symlinks=False)
    except FileNotFoundError:
        pass
    else:
        fail()
    try:
        metadata = os.stat(name, dir_fd=directory, follow_symlinks=False)
    except FileNotFoundError:
        pass
    else:
        if stat.S_ISDIR(metadata.st_mode):
            fail()
        os.unlink(name, dir_fd=directory)
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_CLOEXEC
    transcript = os.open(name, flags, 0o600, dir_fd=directory)
    if not stat.S_ISREG(os.fstat(transcript).st_mode):
        fail()
    os.fchmod(transcript, 0o600)
    identity_temporary_name = f'.{identity_name}.{os.getpid()}.tmp'
    identity_flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_CLOEXEC
    identity = os.open(identity_temporary_name, identity_flags, 0o600, dir_fd=directory)
    identity_bytes = (json.dumps(
        {
            'schema': 'sandbox-e2b-terminal-identity-v1',
            'pid': os.getpid(),
            'terminal_id': terminal_id,
            'operation_id': operation_id,
            'tag': tag,
        },
        separators=(',', ':'),
        sort_keys=True,
    ) + '\n').encode('utf-8')
    write_all(identity, identity_bytes)
    os.fchmod(identity, 0o600)
    identity_metadata = os.fstat(identity)
    if not stat.S_ISREG(identity_metadata.st_mode) or identity_metadata.st_uid != os.geteuid():
        fail()
    os.fsync(identity)
    os.close(identity)
    identity = None
    os.link(
        identity_temporary_name,
        identity_name,
        src_dir_fd=directory,
        dst_dir_fd=directory,
        follow_symlinks=False,
    )
    published_identity = os.stat(identity_name, dir_fd=directory, follow_symlinks=False)
    if (published_identity.st_dev, published_identity.st_ino) != (
        identity_metadata.st_dev,
        identity_metadata.st_ino,
    ):
        fail()
    os.fsync(directory)
    os.unlink(identity_temporary_name, dir_fd=directory)
    identity_temporary_name = None
    os.fsync(directory)
    os.close(directory)
    directory = None
    _, hard_limit = resource.getrlimit(resource.RLIMIT_FSIZE)
    if hard_limit != resource.RLIM_INFINITY and limit > hard_limit:
        fail()
    resource.setrlimit(resource.RLIMIT_FSIZE, (hard_limit, hard_limit))
    shell_command = 'exec /usr/bin/python3 -I -S -c {} {}'.format(
        shlex.quote(SHELL_HELPER),
        shlex.quote(workload_user),
    )
    pipe_reader, pipe_writer = os.pipe2(os.O_CLOEXEC)
    supervisor_pid = os.getpid()
    recorder_pid = os.fork()
    if recorder_pid == 0:
        arm_parent_death(supervisor_pid)
        if pipe_writer != LOG_DESCRIPTOR:
            os.dup2(pipe_writer, LOG_DESCRIPTOR, inheritable=True)
        else:
            os.set_inheritable(pipe_writer, True)
        close_except({0, 1, 2, LOG_DESCRIPTOR})
        os.execv(
            '/usr/bin/script',
            [
                'script',
                '-q',
                '-f',
                '--force',
                '--log-out',
                f'/proc/self/fd/{LOG_DESCRIPTOR}',
                '-c',
                shell_command,
            ],
        )
    os.close(pipe_writer)
    pipe_writer = None
    close_except({pipe_reader, transcript})
    record_transcript(pipe_reader, transcript, limit)
    os.close(pipe_reader)
    pipe_reader = None
    os.close(transcript)
    transcript = None
    _, recorder_status = os.waitpid(recorder_pid, 0)
    recorder_pid = None
    exit_with_status(recorder_status)
except (IndexError, OSError, ValueError):
    if identity_temporary_name is not None and directory is not None:
        try:
            os.unlink(identity_temporary_name, dir_fd=directory)
        except OSError:
            pass
    if recorder_pid not in (None, 0):
        for descriptor in (pipe_reader, pipe_writer):
            if descriptor is not None:
                try:
                    os.close(descriptor)
                except OSError:
                    pass
        stop_recorder(recorder_pid)
    fail()
finally:
    for descriptor in (pipe_reader, pipe_writer, identity, transcript, directory):
        if descriptor is not None:
            try:
                os.close(descriptor)
            except OSError:
                pass
