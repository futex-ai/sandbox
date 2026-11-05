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
TEMPORARY_FILE = 'payload'


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


def create_private_temporary(parent, name):
    parts = relative_parts(name)
    if len(parts) != 1:
        stop(INVALID_PATH)
    opened = None
    created = False
    try:
        os.mkdir(name, mode=0o700, dir_fd=parent)
        created = True
        os.fsync(parent)
        opened = open_below(parent, name, DIRECTORY_FLAGS, FAILED)
        metadata = os.fstat(opened)
        visible = os.stat(name, dir_fd=parent, follow_symlinks=False)
        if (
            not stat.S_ISDIR(metadata.st_mode)
            or metadata.st_uid != os.geteuid()
            or stat.S_IMODE(metadata.st_mode) & 0o077
            or metadata.st_dev != os.fstat(parent).st_dev
            or (visible.st_dev, visible.st_ino) != (metadata.st_dev, metadata.st_ino)
        ):
            stop(FAILED)
        os.fchmod(opened, 0o700)
        return opened, (metadata.st_dev, metadata.st_ino)
    except BaseException:
        if opened is not None:
            os.close(opened)
        if created:
            try:
                os.rmdir(name, dir_fd=parent)
            except OSError:
                pass
        raise


def remove_private_temporary(parent, name, identity):
    try:
        visible = os.stat(name, dir_fd=parent, follow_symlinks=False)
    except FileNotFoundError:
        return
    if not stat.S_ISDIR(visible.st_mode) or (visible.st_dev, visible.st_ino) != identity:
        return
    os.rmdir(name, dir_fd=parent)
    os.fsync(parent)


staging = None
temporary_directory = None
temporary_identity = None
temporary_payload = False
directory = None
state_directory = None
source = None
destination = None
commit_claimed = False
try:
    root = sys.argv[1]
    path = sys.argv[2]
    staging = sys.argv[3]
    temporary_name = sys.argv[4]
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
    temporary_directory, temporary_identity = create_private_temporary(
        directory,
        temporary_name,
    )
    destination_flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_CLOEXEC
    destination = os.open(
        TEMPORARY_FILE,
        destination_flags,
        0o600,
        dir_fd=temporary_directory,
    )
    temporary_payload = True
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
    os.fsync(temporary_directory)
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
    os.replace(
        TEMPORARY_FILE,
        target,
        src_dir_fd=temporary_directory,
        dst_dir_fd=directory,
    )
    temporary_payload = False
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
    remove_private_temporary(directory, temporary_name, temporary_identity)
    os.close(temporary_directory)
    temporary_directory = None
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
    if temporary_directory is not None and temporary_payload and not commit_claimed:
        try:
            os.unlink(TEMPORARY_FILE, dir_fd=temporary_directory)
            temporary_payload = False
            os.fsync(temporary_directory)
        except OSError:
            pass
    if temporary_directory is not None:
        if not temporary_payload and directory is not None and temporary_identity is not None:
            try:
                remove_private_temporary(directory, temporary_name, temporary_identity)
            except OSError:
                pass
        os.close(temporary_directory)
    if directory is not None:
        os.close(directory)
    if staging is not None:
        try:
            os.unlink(staging)
        except OSError:
            pass
