import os
import resource
import stat
import sys

FAILED = 1
DIRECTORY_FLAGS = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC
SHELL_COMMAND = '/bin/bash -c "ulimit -S -f unlimited; exec /bin/bash -il"'


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
    os.dup2(transcript, 2, inheritable=True)
    os.close(transcript)
    transcript = None
    _, hard_limit = resource.getrlimit(resource.RLIMIT_FSIZE)
    resource.setrlimit(resource.RLIMIT_FSIZE, (limit, hard_limit))
    os.execv(
        '/usr/bin/script',
        [
            'script',
            '-q',
            '-f',
            '--force',
            '--log-out',
            '/proc/self/fd/2',
            '-c',
            SHELL_COMMAND,
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
