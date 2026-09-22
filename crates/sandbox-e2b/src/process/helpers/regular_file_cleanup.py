import errno
import hashlib
import os
import stat
import sys

REVOKED = 0
COMMITTED = 51
FAILED = 52
DIRECTORY_FLAGS = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC


def open_below(directory, name, flags):
    try:
        return os.open(name, flags, dir_fd=directory)
    except OSError as error:
        if error.errno in (errno.ELOOP, errno.ENOTDIR):
            raise ValueError()
        raise


def absolute_parts(path):
    parts = [part for part in path.split('/') if part]
    if path != '/' + '/'.join(parts):
        raise ValueError()
    return parts


def relative_parts(path):
    parts = path.split('/')
    if not parts or any(part in ('', '.', '..') for part in parts):
        raise ValueError()
    return parts


def open_absolute_directory(path):
    directory = os.open('/', DIRECTORY_FLAGS)
    try:
        for part in absolute_parts(path):
            child = open_below(directory, part, DIRECTORY_FLAGS)
            os.close(directory)
            directory = child
        return directory
    except Exception:
        os.close(directory)
        raise


def open_private_state(root, path):
    parts = relative_parts(path)
    directory = open_absolute_directory(root)
    metadata = os.fstat(directory)
    if metadata.st_uid != os.geteuid() or stat.S_IMODE(metadata.st_mode) & 0o022:
        os.close(directory)
        raise ValueError()
    try:
        for part in parts[:-1]:
            try:
                os.mkdir(part, mode=0o700, dir_fd=directory)
                os.fsync(directory)
            except FileExistsError:
                pass
            child = open_below(directory, part, DIRECTORY_FLAGS)
            metadata = os.fstat(child)
            if metadata.st_uid != os.geteuid() or stat.S_IMODE(metadata.st_mode) & 0o077:
                os.close(child)
                raise ValueError()
            os.close(directory)
            directory = child
        return directory, parts[-1]
    except Exception:
        os.close(directory)
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


def committed_identity(value, digest):
    parts = value.split(':')
    if len(parts) != 5 or parts[0] != 'commit' or parts[1] != digest:
        raise ValueError()
    owner_uid = int(parts[2])
    owner_gid = int(parts[3])
    mode = int(parts[4])
    if owner_uid < 0 or owner_gid < 0 or mode < 0 or mode & ~0o7777:
        raise ValueError()
    return owner_uid, owner_gid, mode


def finish_target(directory, target, expected, digest, owner_uid, owner_gid, mode):
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
        if observed.hexdigest() != digest:
            return False
        if (metadata.st_uid, metadata.st_gid) != (owner_uid, owner_gid):
            os.fchown(opened, owner_uid, owner_gid)
        os.fchmod(opened, mode)
        os.fsync(opened)
        return True
    finally:
        os.close(opened)


directory = None
state_directory = None
outcome = FAILED
try:
    root = sys.argv[1]
    path = sys.argv[2]
    staging = sys.argv[3]
    temporary = sys.argv[4]
    state_root = sys.argv[5]
    state_path = sys.argv[6]
    expected = int(sys.argv[7])
    expected_digest = sys.argv[8]
    if len(expected_digest) != 64 or any(
        character not in '0123456789abcdef' for character in expected_digest
    ):
        raise ValueError()
    state_directory, state_name = open_private_state(state_root, state_path)
    try:
        os.symlink('revoked', state_name, dir_fd=state_directory)
        state_value = 'revoked'
    except FileExistsError:
        state_value = os.readlink(state_name, dir_fd=state_directory)
    os.fsync(state_directory)
    try:
        os.unlink(staging)
    except FileNotFoundError:
        pass
    path_parts = relative_parts(path)
    directory = open_absolute_directory(root)
    for part in path_parts[:-1]:
        child = open_below(directory, part, DIRECTORY_FLAGS)
        os.close(directory)
        directory = child
    target = path_parts[-1]
    if state_value == 'revoked':
        try:
            os.unlink(temporary, dir_fd=directory)
        except FileNotFoundError:
            pass
        os.fsync(directory)
        if target_matches(directory, target, expected, expected_digest):
            outcome = COMMITTED
        else:
            outcome = REVOKED
    elif state_value.startswith('commit:'):
        owner_uid, owner_gid, mode = committed_identity(state_value, expected_digest)
        if target_matches(directory, temporary, expected, expected_digest):
            try:
                os.replace(temporary, target, src_dir_fd=directory, dst_dir_fd=directory)
            except FileNotFoundError:
                pass
        else:
            try:
                os.unlink(temporary, dir_fd=directory)
            except FileNotFoundError:
                pass
        if finish_target(
            directory,
            target,
            expected,
            expected_digest,
            owner_uid,
            owner_gid,
            mode,
        ):
            outcome = COMMITTED
        os.fsync(directory)
    else:
        raise ValueError()
except (IndexError, OSError, ValueError):
    outcome = FAILED
finally:
    if directory is not None:
        os.close(directory)
    if state_directory is not None:
        os.close(state_directory)
raise SystemExit(outcome)
