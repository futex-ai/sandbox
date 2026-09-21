import errno
import hashlib
import os
import stat
import sys

REVOKED = 0
COMMITTED = 51
FAILED = 52


def open_below(directory, name, flags):
    try:
        return os.open(name, flags, dir_fd=directory)
    except OSError as error:
        if error.errno in (errno.ELOOP, errno.ENOTDIR):
            raise ValueError()
        raise


def target_matches(directory, target, expected, digest):
    flags = os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC
    try:
        opened = os.open(target, flags, dir_fd=directory)
    except OSError:
        return False
    try:
        metadata = os.fstat(opened)
        if not stat.S_ISREG(metadata.st_mode) or metadata.st_size != expected:
            return False
        observed = hashlib.sha256()
        while True:
            chunk = os.read(opened, 65536)
            if not chunk:
                break
            observed.update(chunk)
        return observed.hexdigest() == digest
    finally:
        os.close(opened)


directory = None
outcome = FAILED
try:
    root = sys.argv[1]
    path = sys.argv[2]
    staging = sys.argv[3]
    temporary = sys.argv[4]
    state = sys.argv[5]
    expected = int(sys.argv[6])
    try:
        os.symlink('revoked', state)
        state_value = 'revoked'
    except FileExistsError:
        state_value = os.readlink(state)
    try:
        os.unlink(staging)
    except FileNotFoundError:
        pass
    root_parts = [part for part in root.split('/') if part]
    path_parts = path.split('/')
    if root != '/' + '/'.join(root_parts):
        raise ValueError()
    if not path_parts or any(part in ('', '.', '..') for part in path_parts):
        raise ValueError()
    flags = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC
    directory = os.open('/', flags)
    for part in root_parts + path_parts[:-1]:
        child = open_below(directory, part, flags)
        os.close(directory)
        directory = child
    target = path_parts[-1]
    if state_value == 'revoked':
        try:
            os.unlink(temporary, dir_fd=directory)
        except FileNotFoundError:
            pass
        outcome = REVOKED
    elif state_value.startswith('commit:'):
        digest = state_value.removeprefix('commit:')
        if len(digest) != 64 or any(character not in '0123456789abcdef' for character in digest):
            raise ValueError()
        try:
            os.replace(temporary, target, src_dir_fd=directory, dst_dir_fd=directory)
        except FileNotFoundError:
            pass
        os.fsync(directory)
        if target_matches(directory, target, expected, digest):
            outcome = COMMITTED
    else:
        raise ValueError()
except (IndexError, OSError, ValueError):
    outcome = FAILED
finally:
    if directory is not None:
        os.close(directory)
raise SystemExit(outcome)
