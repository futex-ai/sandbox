import os
import stat
import sys

FAILED = 1
DIRECTORY_FLAGS = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC


def fail():
    os._exit(FAILED)


def absolute_parts(path):
    if not path.startswith('/') or path == '/' or os.path.normpath(path) != path:
        fail()
    parts = path.split('/')[1:]
    if not parts or any(part in ('', '.', '..') for part in parts):
        fail()
    return parts


def relative_parts(path):
    parts = path.split('/')
    if not parts or any(part in ('', '.', '..') for part in parts):
        fail()
    return parts


try:
    root = sys.argv[1]
    relative = sys.argv[2]
    directory = os.open('/', DIRECTORY_FLAGS)
    try:
        for part in absolute_parts(root):
            child = os.open(part, DIRECTORY_FLAGS, dir_fd=directory)
            os.close(directory)
            directory = child
        root_metadata = os.fstat(directory)
        if root_metadata.st_uid != os.geteuid() or stat.S_IMODE(root_metadata.st_mode) & 0o022:
            fail()
        for part in relative_parts(relative):
            try:
                os.mkdir(part, mode=0o700, dir_fd=directory)
                os.fsync(directory)
            except FileExistsError:
                pass
            child = os.open(part, DIRECTORY_FLAGS, dir_fd=directory)
            metadata = os.fstat(child)
            if metadata.st_uid != os.geteuid() or stat.S_IMODE(metadata.st_mode) & 0o077:
                os.close(child)
                fail()
            os.close(directory)
            directory = child
    finally:
        os.close(directory)
except (IndexError, OSError, ValueError):
    fail()
