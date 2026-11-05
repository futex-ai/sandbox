import contextlib
import fcntl
import grp
import os
import pwd
import stat
import subprocess
import sys

FLAGS = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC


def directory(path, owner, private, create=False):
    parts = path.split('/')[1:]
    if not path.startswith('/') or any(p in ('', '.', '..') for p in parts):
        raise ValueError()
    fd = os.open('/', FLAGS)
    try:
        for index, part in enumerate(parts):
            last = index == len(parts) - 1
            if create and last:
                try:
                    os.mkdir(part, 0o700, dir_fd=fd)
                    os.fsync(fd)
                except FileExistsError:
                    pass
            child = os.open(part, FLAGS, dir_fd=fd)
            os.close(fd)
            fd = child
            info = os.fstat(fd)
            if last:
                if info.st_uid != owner or stat.S_IMODE(info.st_mode) & (0o077 if private else 0o022):
                    raise ValueError()
            elif info.st_uid not in (0, owner) or stat.S_IMODE(info.st_mode) & 0o022:
                if not (info.st_uid == 0 and stat.S_IMODE(info.st_mode) & stat.S_ISVTX):
                    raise ValueError()
        return fd
    except BaseException:
        os.close(fd)
        raise


@contextlib.contextmanager
def initialization_lock(state_fd):
    """Keep shared account and directory setup atomic through workspace validation."""
    fd = os.open('bootstrap-lock', os.O_RDWR | os.O_CREAT | os.O_NOFOLLOW | os.O_CLOEXEC,
                 0o600, dir_fd=state_fd)
    try:
        info = os.fstat(fd)
        if not stat.S_ISREG(info.st_mode) or info.st_uid != 0:
            raise ValueError()
        fcntl.flock(fd, fcntl.LOCK_EX)
        yield
    finally:
        os.close(fd)


@contextlib.contextmanager
def attempt_allocation(attempt_fd):
    """Bootstrap takes shared initialization before this short directory lock.
    Abandon takes only this lock and can unlink a queued attempt directory.
    Its retained descriptor then has no links and forbids later allocation.
    """
    fcntl.flock(attempt_fd, fcntl.LOCK_EX)
    try:
        if os.fstat(attempt_fd).st_nlink == 0:
            raise ValueError()
        yield
    finally:
        fcntl.flock(attempt_fd, fcntl.LOCK_UN)


def prepare(home, state, workspace, controller_name, workload_name, attempt):
    if os.geteuid() != 0 or len(attempt) != 32 or any(c not in '0123456789abcdef' for c in attempt):
        raise ValueError()
    state_fd = directory(state, 0, True, True)
    attempt_fd = None
    try:
        os.mkdir(attempt, 0o700, dir_fd=state_fd)
        os.fsync(state_fd)
        attempt_fd = os.open(attempt, FLAGS, dir_fd=state_fd)
        with initialization_lock(state_fd):
            with attempt_allocation(attempt_fd):
                pass
            initialize(home, workspace, controller_name, workload_name, attempt, attempt_fd)
    finally:
        if attempt_fd is not None:
            os.close(attempt_fd)
        os.close(state_fd)


def initialize(home, workspace, controller_name, workload_name, attempt, attempt_fd):
    controller = pwd.getpwnam(controller_name)
    if workload_name == controller_name or workload_name == 'root':
        raise ValueError()
    try:
        workload = pwd.getpwnam(workload_name)
    except KeyError:
        if workload_name != 'sandbox-workload':
            raise ValueError()
        subprocess.run(['/usr/sbin/useradd', '--system', '--user-group', '--no-create-home',
                        '--home-dir', workspace, '--shell', '/usr/sbin/nologin', workload_name],
                       check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        workload = pwd.getpwnam(workload_name)
    if workload.pw_uid in (0, controller.pw_uid) or workload.pw_gid in (0, controller.pw_gid):
        raise ValueError()
    if os.getgrouplist(workload_name, workload.pw_gid) != [workload.pw_gid]:
        raise ValueError()
    group = grp.getgrgid(workload.pw_gid)
    if group.gr_name not in (workload_name, 'nogroup') or group.gr_mem:
        raise ValueError()
    if group.gr_name in ('root', 'sudo', 'wheel', 'disk', 'docker', 'lxd', 'incus', 'shadow'):
        raise ValueError()
    if any(account.pw_uid != workload.pw_uid and account.pw_gid == workload.pw_gid
           for account in pwd.getpwall()):
        raise ValueError()
    sudo = subprocess.run(['/usr/bin/sudo', '-n', '-l', '-U', workload_name],
                          stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                          env={'PATH': '/usr/bin:/bin', 'LC_ALL': 'C'})
    denied = ('User ' + workload_name + ' is not allowed to run sudo on ').encode('ascii')
    if sudo.returncode not in (0, 1) or not sudo.stdout.startswith(denied) or sudo.stderr:
        raise ValueError()
    home_fd = directory(home, controller.pw_uid, False)
    os.fchmod(home_fd, 0o700)
    os.close(home_fd)
    stage = home + '/.sandbox-boat'
    try:
        os.mkdir(stage, 0o700)
        os.chown(stage, controller.pw_uid, controller.pw_gid)
    except FileExistsError:
        pass
    stage_fd = directory(stage, controller.pw_uid, True)
    try:
        try:
            os.mkdir(workspace, 0o755)
            os.chown(workspace, workload.pw_uid, workload.pw_gid)
        except FileExistsError:
            pass
        workspace_fd = os.open(workspace, FLAGS)
        try:
            info = os.fstat(workspace_fd)
            if info.st_uid not in (0, workload.pw_uid):
                raise ValueError()
        finally:
            os.close(workspace_fd)
        with attempt_allocation(attempt_fd):
            os.mkdir(attempt, 0o700, dir_fd=stage_fd)
            os.chown(attempt, controller.pw_uid, controller.pw_gid, dir_fd=stage_fd,
                     follow_symlinks=False)
            os.fsync(stage_fd)
    finally:
        os.close(stage_fd)


if __name__ == '__main__':
    try:
        prepare(*sys.argv[1:])
    except BaseException:
        raise SystemExit(49)
