import os
import sys

FAILED = 1
DIRECTORY_FLAGS = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC


def fail():
    os._exit(FAILED)


def path_parts(path):
    if not path.startswith("/") or path == "/" or os.path.normpath(path) != path:
        fail()
    parts = path.split("/")[1:]
    if not parts or any(part in ("", ".", "..") for part in parts):
        fail()
    return parts


try:
    directory = os.open("/", DIRECTORY_FLAGS)
    try:
        for part in path_parts(sys.argv[1]):
            try:
                os.mkdir(part, mode=0o700, dir_fd=directory)
            except FileExistsError:
                pass
            child = os.open(part, DIRECTORY_FLAGS, dir_fd=directory)
            os.close(directory)
            directory = child
    finally:
        os.close(directory)
except (IndexError, OSError, ValueError):
    fail()
