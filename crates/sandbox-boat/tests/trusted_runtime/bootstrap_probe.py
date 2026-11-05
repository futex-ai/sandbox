"""Drive the complete production entry point inside a disposable private root."""


def owner(path):
    info = os.stat(path)
    return [info.st_uid, info.st_gid, stat.S_IMODE(info.st_mode)]


def concurrent(source, scenario):
    actors = []
    try:
        first = launch(source, scenario, 'first', IDS[0])
        actors.append(first)
        phase = first.event()['phase']
        expected = 'missing' if scenario == 'account' else 'stage-created'
        if phase != expected:
            raise ValueError('first actor reached the wrong barrier: ' + phase)
        second = launch(source, scenario, 'second', IDS[1])
        actors.append(second)
        phase = second.event()['phase']
        if scenario == 'staging' and phase == 'stage':
            second_exit = second.finish()
            first.release()
            first_exit = first.finish()
        else:
            first.release()
            first_exit = first.finish()
            if phase == 'missing':
                second.release()
            second_exit = second.finish()
        third = launch(source, scenario, 'third', IDS[2])
        actors.append(third)
        third_exit = third.finish()
        with open('/accounts.json') as account_file:
            account = json.load(account_file)
        attempts = []
        for attempt in IDS:
            stage = HOME + '/.sandbox-boat/' + attempt
            state = STATE + '/' + attempt
            if os.path.isdir(stage) and os.path.isdir(state):
                attempts.append(owner(stage) + owner(state))
        return {'statuses': [first_exit, second_exit, third_exit],
                'creations': account['creations'], 'account': [account['uid'], account['uid']],
                'workspace': owner(WORKSPACE)[:2], 'stage': owner(HOME + '/.sandbox-boat'),
                'attempts': attempts}
    finally:
        for actor in actors:
            actor.cancel()


def cancelled(source, abandoned_source, scenario):
    os.mkdir(STATE, 0o700)
    if scenario != 'cancel-no-stage':
        os.mkdir(HOME + '/.sandbox-boat', 0o700)
        os.chown(HOME + '/.sandbox-boat', CONTROLLER_ID, CONTROLLER_ID)
    actors = []
    lock = None
    try:
        waiting = launch(source, scenario, 'cancel', IDS[0])
        actors.append(waiting)
        event = waiting.event()
        if event['phase'] != 'lock':
            raise ValueError('queued bootstrap did not reach its shared lock')
        lock_path = next(STATE + '/' + name for name in os.listdir(STATE)
                         if os.stat(STATE + '/' + name).st_ino == event['inode'])
        lock = os.open(lock_path, os.O_RDWR | os.O_NOFOLLOW | os.O_CLOEXEC)
        fcntl.flock(lock, fcntl.LOCK_EX)
        waiting.release()
        abandoned = []
        for _ in range(2):
            cleanup = launch(abandoned_source, 'abandon', 'abandon', IDS[0])
            actors.append(cleanup)
            abandoned.append(cleanup.finish())
        fcntl.flock(lock, fcntl.LOCK_UN)
        status = waiting.finish()
        followup = launch(source, scenario, 'third', IDS[2])
        actors.append(followup)
        followup_status = followup.finish()
        remaining = sum(os.path.exists(path + '/' + IDS[0])
                        for path in [HOME + '/.sandbox-boat', STATE])
        return {'status': status, 'abandoned': abandoned, 'remaining': remaining,
                'followup': followup_status, 'lock': owner(lock_path),
                'lock_preserved': os.stat(lock_path).st_ino == event['inode']}
    finally:
        if lock is not None:
            os.close(lock)
        for actor in actors:
            actor.cancel()


def probe(source, scenario):
    os.makedirs(HOME)
    os.makedirs('/var/lib')
    os.chown(HOME, CONTROLLER_ID, CONTROLLER_ID)
    with open('/accounts.json', 'w') as target:
        json.dump({'uid': None if scenario == 'account' else WORKLOAD_ID, 'creations': 0}, target)
    if scenario.startswith('cancel'):
        return cancelled(source, abandoned_source, scenario)
    return concurrent(source, scenario)


source, abandoned_source, root, scenario = sys.argv[1:]
codecs.lookup('ascii')
pid = os.fork()
if pid == 0:
    try:
        os.chown(root, 0, 0)
        os.chmod(root, 0o755)
        os.chroot(root)
        os.chdir('/')
        print(json.dumps(probe(source, scenario)), flush=True)
    except BaseException:
        traceback.print_exc()
        os._exit(1)
    os._exit(0)
try:
    _, status = os.waitpid(pid, 0)
    raise SystemExit(os.waitstatus_to_exitcode(status))
finally:
    shutil.rmtree(root)
