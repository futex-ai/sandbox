import os
import resource
import shlex
import stat
import sys

FAILED = 1
LOG_DESCRIPTOR = 3
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
transcript = None
try:
    path = sys.argv[1]
    limit = int(sys.argv[2])
    workload_user = sys.argv[3]
    if limit < 0:
        fail()
    directory, name = open_parent(path)
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
    os.close(directory)
    directory = None
    if transcript != LOG_DESCRIPTOR:
        os.dup2(transcript, LOG_DESCRIPTOR, inheritable=True)
        os.close(transcript)
    else:
        os.set_inheritable(transcript, True)
    transcript = None
    _, hard_limit = resource.getrlimit(resource.RLIMIT_FSIZE)
    resource.setrlimit(resource.RLIMIT_FSIZE, (limit, hard_limit))
    shell_command = 'exec /usr/bin/python3 -I -S -c {} {}'.format(
        shlex.quote(SHELL_HELPER),
        shlex.quote(workload_user),
    )
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
except (IndexError, OSError, ValueError):
    fail()
finally:
    for descriptor in (transcript, directory):
        if descriptor is not None:
            try:
                os.close(descriptor)
            except OSError:
                pass
