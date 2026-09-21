import os
import stat
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


def open_parent(parts):
    directory = os.open("/", DIRECTORY_FLAGS)
    try:
        for part in parts[:-1]:
            try:
                child = os.open(part, DIRECTORY_FLAGS, dir_fd=directory)
            except FileNotFoundError:
                os.close(directory)
                return None
            os.close(directory)
            directory = child
        return directory
    except Exception:
        os.close(directory)
        raise


def remove_entry(parent, name):
    try:
        metadata = os.stat(name, dir_fd=parent, follow_symlinks=False)
    except FileNotFoundError:
        return
    if not stat.S_ISDIR(metadata.st_mode):
        os.unlink(name, dir_fd=parent)
        return
    child = os.open(name, DIRECTORY_FLAGS, dir_fd=parent)
    opened = os.fstat(child)
    if (opened.st_dev, opened.st_ino) != (metadata.st_dev, metadata.st_ino):
        fail()
    try:
        for entry in os.listdir(child):
            remove_entry(child, entry)
    finally:
        os.close(child)
    current = os.stat(name, dir_fd=parent, follow_symlinks=False)
    if (current.st_dev, current.st_ino) != (opened.st_dev, opened.st_ino):
        fail()
    os.rmdir(name, dir_fd=parent)


try:
    for target in sys.argv[1:]:
        parts = path_parts(target)
        parent = open_parent(parts)
        if parent is None:
            continue
        try:
            remove_entry(parent, parts[-1])
        finally:
            os.close(parent)
except (OSError, ValueError):
    fail()
