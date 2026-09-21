import errno
import hashlib
import os
import stat
import sys

INVALID_ROOT = 45
NOT_REGULAR = 46
TOO_LARGE = 47
INVALID_PATH = 48
FAILED = 49
REVOKED = 50
STAGING_MISMATCH = 51


def stop(code):
    raise SystemExit(code)


def open_below(directory, name, flags, failure):
    try:
        return os.open(name, flags, dir_fd=directory)
    except OSError as error:
        if error.errno in (errno.ELOOP, errno.ENOTDIR):
            stop(NOT_REGULAR)
        stop(failure)


staging = None
temporary = None
directory = None
source = None
destination = None
commit_claimed = False
try:
    root = sys.argv[1]
    path = sys.argv[2]
    staging = sys.argv[3]
    temporary = sys.argv[4]
    state = sys.argv[5]
    expected = int(sys.argv[6])
    expected_digest = sys.argv[7]
    transfer_limit = int(sys.argv[8])
    if len(expected_digest) != 64 or any(
        character not in '0123456789abcdef' for character in expected_digest
    ):
        stop(INVALID_PATH)
    root_parts = [part for part in root.split('/') if part]
    path_parts = path.split('/')
    if root != '/' + '/'.join(root_parts):
        stop(INVALID_ROOT)
    if not path_parts or any(part in ('', '.', '..') for part in path_parts):
        stop(INVALID_PATH)
    if expected < 0 or expected > transfer_limit:
        stop(TOO_LARGE)
    directory_flags = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC
    directory = os.open('/', directory_flags)
    for part in root_parts:
        child = open_below(directory, part, directory_flags, INVALID_ROOT)
        os.close(directory)
        directory = child
    for part in path_parts[:-1]:
        child = open_below(directory, part, directory_flags, INVALID_PATH)
        os.close(directory)
        directory = child
    target = path_parts[-1]
    try:
        target_metadata = os.stat(target, dir_fd=directory, follow_symlinks=False)
    except FileNotFoundError:
        mode = 0o600
    except OSError:
        stop(FAILED)
    else:
        if not stat.S_ISREG(target_metadata.st_mode):
            stop(NOT_REGULAR)
        mode = stat.S_IMODE(target_metadata.st_mode)
    source_flags = os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC
    source = os.open(staging, source_flags)
    source_metadata = os.fstat(source)
    if not stat.S_ISREG(source_metadata.st_mode):
        stop(NOT_REGULAR)
    if source_metadata.st_size != expected:
        stop(FAILED)
    os.unlink(staging)
    staging = None
    destination_flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_CLOEXEC
    destination = os.open(temporary, destination_flags, mode, dir_fd=directory)
    os.fchmod(destination, mode)
    digest = hashlib.sha256()
    remaining = expected
    while remaining:
        chunk = os.read(source, min(remaining, 65536))
        if not chunk:
            stop(FAILED)
        digest.update(chunk)
        view = memoryview(chunk)
        while view:
            written = os.write(destination, view)
            view = view[written:]
        remaining -= len(chunk)
    if os.read(source, 1):
        stop(FAILED)
    observed_digest = digest.hexdigest()
    if observed_digest != expected_digest:
        stop(STAGING_MISMATCH)
    os.fsync(destination)
    os.close(destination)
    destination = None
    commit_state = 'commit:' + expected_digest
    try:
        os.symlink(commit_state, state)
    except FileExistsError:
        if os.readlink(state) != commit_state:
            stop(REVOKED)
    commit_claimed = True
    os.replace(temporary, target, src_dir_fd=directory, dst_dir_fd=directory)
    temporary = None
    os.fsync(directory)
except (IndexError, ValueError):
    stop(INVALID_PATH)
except OSError:
    stop(FAILED)
finally:
    if destination is not None:
        os.close(destination)
    if source is not None:
        os.close(source)
    if temporary is not None and directory is not None and not commit_claimed:
        try:
            os.unlink(temporary, dir_fd=directory)
        except OSError:
            pass
    if directory is not None:
        os.close(directory)
    if staging is not None:
        try:
            os.unlink(staging)
        except OSError:
            pass
