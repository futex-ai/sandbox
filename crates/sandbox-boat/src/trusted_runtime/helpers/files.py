def confined_file(root, path):
    root_parts = [] if root == '/' else root.split('/')[1:]
    if root != '/' + '/'.join(root_parts) or any(p in ('', '.', '..') for p in root_parts):
        raise SystemExit(45)
    parts = path.split('/')
    if not parts or any(p in ('', '.', '..') for p in parts):
        raise SystemExit(48)
    directory = os.open('/', DIRECTORY_FLAGS)
    try:
        for part in root_parts + parts[:-1]:
            child = os.open(part, DIRECTORY_FLAGS, dir_fd=directory)
            os.close(directory)
            directory = child
        fd = os.open(parts[-1], FILE_FLAGS, dir_fd=directory)
        return fd
    finally:
        os.close(directory)


def export_file():
    fd = confined_file(manifest['root'], manifest['path'])
    try:
        info = os.fstat(fd)
        if not stat.S_ISREG(info.st_mode):
            raise SystemExit(46)
        if info.st_size > TRANSFER_LIMIT:
            raise SystemExit(47)
        position = min(manifest['offset'], info.st_size)
        remaining = min(manifest['max_bytes'], info.st_size - position)
        digest = hashlib.sha256()
        sizes = []
        while remaining:
            chunk = os.pread(fd, min(remaining, CHUNK), position)
            if not chunk:
                raise OSError()
            write_at(stage, 'chunk-' + str(len(sizes)), chunk, controller_uid, controller_gid)
            digest.update(chunk)
            sizes.append(len(chunk))
            position += len(chunk)
            remaining -= len(chunk)
        result = {'version': 1, 'attempt': attempt, 'total_size': info.st_size,
                  'sizes': sizes, 'digest': digest.hexdigest()}
        print(json.dumps(result, separators=(',', ':')), end='')
    finally:
        os.close(fd)


def assemble():
    expected = manifest['expected_size']
    if expected < 0 or expected > TRANSFER_LIMIT:
        raise SystemExit(47)
    remaining = expected
    digest = hashlib.sha256()
    destination = os.open('payload', os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW |
                          os.O_CLOEXEC, 0o600, dir_fd=state_fd)
    try:
        for index in range((expected + CHUNK - 1) // CHUNK):
            chunk = read_file_at(stage, 'chunk-' + str(index), min(remaining, CHUNK))
            if len(chunk) != min(remaining, CHUNK):
                raise SystemExit(51)
            digest.update(chunk)
            remaining -= len(chunk)
            view = memoryview(chunk)
            while view:
                view = view[os.write(destination, view):]
        if remaining or digest.hexdigest() != manifest['expected_digest']:
            raise SystemExit(51)
        os.fsync(destination)
    finally:
        os.close(destination)
    os.fsync(state_fd)


def writer(cleanup):
    saved = sys.argv
    try:
        sys.argv = ['trusted-writer', manifest['root'], manifest['path'],
                    state_root + '/' + attempt + '/payload', '.sandbox-boat-write-' + attempt,
                    state_root, 'fences/' + attempt, str(manifest['expected_size']),
                    manifest['expected_digest'], workload.pw_name,
                    str(manifest['retain_fence']).lower() if cleanup else str(TRANSFER_LIMIT)]
        name = 'cleanup' if cleanup else 'writer'
        exec(compile(base64.b64decode(bundle['sources'][name], validate=True),
                     '<trusted-file-' + name + '>', 'exec'), {'__name__': '__main__'})
    finally:
        sys.argv = saved


def dispatch():
    try:
        if action == 'supervise':
            supervise()
        elif action == 'read':
            export_file()
        elif action == 'assemble':
            assemble()
        elif action == 'write':
            writer(False)
        elif action in ('reconcile', 'reconcile-resolved'):
            manifest['retain_fence'] = action == 'reconcile'
            writer(True)
        else:
            control()
    except FileNotFoundError:
        raise SystemExit(44)
    except OSError as error:
        if error.errno in (errno.ELOOP, errno.ENOTDIR):
            raise SystemExit(46)
        raise SystemExit(49)
