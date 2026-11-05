import fcntl
import os
import stat
import sys

FLAGS = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC
STAGE = '/home/user/.sandbox-boat'
STATE = '/var/lib/sandbox-boat'


def directory(path):
    fd = os.open('/', FLAGS)
    try:
        for part in path.split('/')[1:]:
            child = os.open(part, FLAGS, dir_fd=fd)
            os.close(fd)
            fd = child
        return fd
    except BaseException:
        os.close(fd)
        raise


def remove(root, attempt):
    try:
        parent = directory(root)
    except FileNotFoundError:
        return
    try:
        try:
            child = os.open(attempt, FLAGS, dir_fd=parent)
        except FileNotFoundError:
            return
        try:
            for name in os.listdir(child):
                info = os.stat(name, dir_fd=child, follow_symlinks=False)
                if not stat.S_ISREG(info.st_mode):
                    raise ValueError()
                os.unlink(name, dir_fd=child)
            os.fsync(child)
        finally:
            os.close(child)
        os.rmdir(attempt, dir_fd=parent)
        os.fsync(parent)
    finally:
        os.close(parent)


def abandon(attempt):
    """Lock only the attempt directory, without waiting for shared initialization."""
    if len(attempt) != 32 or any(c not in '0123456789abcdef' for c in attempt):
        raise ValueError()
    attempt_fd = None
    try:
        try:
            state_fd = directory(STATE)
        except FileNotFoundError:
            state_fd = None
        if state_fd is not None:
            try:
                try:
                    attempt_fd = os.open(attempt, FLAGS, dir_fd=state_fd)
                except FileNotFoundError:
                    pass
            finally:
                os.close(state_fd)
        if attempt_fd is not None:
            info = os.fstat(attempt_fd)
            if info.st_uid != 0 or stat.S_IMODE(info.st_mode) & 0o077:
                raise ValueError()
            fcntl.flock(attempt_fd, fcntl.LOCK_EX)
        remove(STAGE, attempt)
        remove(STATE, attempt)
    finally:
        if attempt_fd is not None:
            os.close(attempt_fd)


try:
    abandon(sys.argv[1])
except BaseException:
    raise SystemExit(49)
