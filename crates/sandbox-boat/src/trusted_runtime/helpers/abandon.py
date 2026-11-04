import os
import stat
import sys

attempt = sys.argv[1]
if len(attempt) != 32 or any(c not in '0123456789abcdef' for c in attempt):
    raise SystemExit(49)
flags = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC
for root in ['/home/user/.sandbox-boat', '/var/lib/sandbox-boat']:
    directory = os.open('/', flags)
    for part in root.split('/')[1:]:
        child = os.open(part, flags, dir_fd=directory)
        os.close(directory)
        directory = child
    try:
        child = os.open(attempt, flags, dir_fd=directory)
    except FileNotFoundError:
        os.close(directory)
        continue
    try:
        for name in os.listdir(child):
            info = os.stat(name, dir_fd=child, follow_symlinks=False)
            if not stat.S_ISREG(info.st_mode):
                raise SystemExit(49)
            os.unlink(name, dir_fd=child)
        os.fsync(child)
    finally:
        os.close(child)
    os.rmdir(attempt, dir_fd=directory)
    os.fsync(directory)
    os.close(directory)
