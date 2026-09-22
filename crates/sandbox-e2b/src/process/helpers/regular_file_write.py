import errno
import hashlib
import os
import pwd
import stat
import sys

INVALID_ROOT = 45
NOT_REGULAR = 46
TOO_LARGE = 47
INVALID_PATH = 48
FAILED = 49
REVOKED = 50
STAGING_MISMATCH = 51
DIRECTORY_FLAGS = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC


def stop(code):
    raise SystemExit(code)


def open_below(directory, name, flags, failure):
    try:
        return os.open(name, flags, dir_fd=directory)
    except OSError as error:
        if error.errno in (errno.ELOOP, errno.ENOTDIR):
            stop(NOT_REGULAR)
        stop(failure)


def absolute_parts(path, failure):
    parts = [part for part in path.split('/') if part]
    if path != '/' + '/'.join(parts):
        stop(failure)
    return parts


def relative_parts(path):
    parts = path.split('/')
    if not parts or any(part in ('', '.', '..') for part in parts):
        stop(INVALID_PATH)
    return parts


def open_absolute_directory(path, failure):
    directory = os.open('/', DIRECTORY_FLAGS)
    for part in absolute_parts(path, failure):
        child = open_below(directory, part, DIRECTORY_FLAGS, failure)
        os.close(directory)
        directory = child
    return directory


def open_private_state(root, path):
    parts = relative_parts(path)
    directory = open_absolute_directory(root, INVALID_PATH)
    metadata = os.fstat(directory)
    if metadata.st_uid != os.geteuid() or stat.S_IMODE(metadata.st_mode) & 0o022:
        os.close(directory)
        stop(FAILED)
    for part in parts[:-1]:
        try:
            os.mkdir(part, mode=0o700, dir_fd=directory)
            os.fsync(directory)
        except FileExistsError:
            pass
        child = open_below(directory, part, DIRECTORY_FLAGS, FAILED)
        metadata = os.fstat(child)
        if metadata.st_uid != os.geteuid() or stat.S_IMODE(metadata.st_mode) & 0o077:
            os.close(child)
            os.close(directory)
            stop(FAILED)
        os.close(directory)
        directory = child
    return directory, parts[-1]


staging = None
temporary = None
directory = None
state_directory = None
source = None
destination = None
commit_claimed = False
try:
    root = sys.argv[1]
    path = sys.argv[2]
    staging = sys.argv[3]
    temporary = sys.argv[4]
    state_root = sys.argv[5]
    state_path = sys.argv[6]
    expected = int(sys.argv[7])
    expected_digest = sys.argv[8]
    workload_user = sys.argv[9]
    transfer_limit = int(sys.argv[10])
    if len(expected_digest) != 64 or any(
        character not in '0123456789abcdef' for character in expected_digest
    ):
        stop(INVALID_PATH)
    path_parts = relative_parts(path)
    if expected < 0 or expected > transfer_limit:
        stop(TOO_LARGE)
    directory = open_absolute_directory(root, INVALID_ROOT)
    for part in path_parts[:-1]:
        child = open_below(directory, part, DIRECTORY_FLAGS, INVALID_PATH)
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
    if workload_user:
        try:
            account = pwd.getpwnam(workload_user)
        except KeyError:
            stop(FAILED)
        if account.pw_uid == 0:
            stop(FAILED)
        owner_uid = account.pw_uid
        owner_gid = account.pw_gid
    else:
        owner_uid = source_metadata.st_uid
        owner_gid = source_metadata.st_gid
    os.unlink(staging)
    staging = None
    destination_flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_CLOEXEC
    destination = os.open(temporary, destination_flags, 0o600, dir_fd=directory)
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
    if digest.hexdigest() != expected_digest:
        stop(STAGING_MISMATCH)
    os.fsync(destination)
    state_directory, state_name = open_private_state(state_root, state_path)
    commit_state = 'commit:{}:{}:{}:{}'.format(
        expected_digest,
        owner_uid,
        owner_gid,
        mode,
    )
    try:
        os.symlink(commit_state, state_name, dir_fd=state_directory)
    except FileExistsError:
        if os.readlink(state_name, dir_fd=state_directory) != commit_state:
            stop(REVOKED)
    os.fsync(state_directory)
    commit_claimed = True
    os.replace(temporary, target, src_dir_fd=directory, dst_dir_fd=directory)
    temporary = None
    destination_metadata = os.fstat(destination)
    visible_metadata = os.stat(target, dir_fd=directory, follow_symlinks=False)
    if (visible_metadata.st_dev, visible_metadata.st_ino) != (
        destination_metadata.st_dev,
        destination_metadata.st_ino,
    ):
        stop(FAILED)
    if (destination_metadata.st_uid, destination_metadata.st_gid) != (owner_uid, owner_gid):
        os.fchown(destination, owner_uid, owner_gid)
    os.fchmod(destination, mode)
    os.fsync(destination)
    os.close(destination)
    destination = None
    os.fsync(directory)
    os.unlink(state_name, dir_fd=state_directory)
    os.fsync(state_directory)
except (IndexError, ValueError):
    stop(INVALID_PATH)
except OSError:
    stop(FAILED)
finally:
    if destination is not None:
        os.close(destination)
    if source is not None:
        os.close(source)
    if state_directory is not None:
        os.close(state_directory)
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
