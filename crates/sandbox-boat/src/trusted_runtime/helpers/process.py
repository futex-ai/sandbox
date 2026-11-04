def supervise():
    write_at(state_fd, 'claimed', attempt.encode('ascii'))
    until = time.monotonic() + manifest['timeout_ms'] / 1000
    child = None
    finished = False
    try:
        emit('hello')
        wait_marker('release', until)
        if marker('cancel'):
            return
        environment = {'PATH': '/usr/local/bin:/usr/bin:/bin', 'HOME': workload.pw_dir,
                       'USER': workload.pw_name, 'LOGNAME': workload.pw_name}
        environment.update(manifest['envs'])
        with child_lock():
            if marker('cancel'):
                return
            child = subprocess.Popen([manifest['command'], *manifest['args']],
                                     cwd=manifest['cwd'] or bundle['workspace'], env=environment,
                                     stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                                     stderr=subprocess.PIPE, close_fds=True, start_new_session=True,
                                     preexec_fn=handoff)
            identity = {'attempt': attempt, 'pid': child.pid, 'start': child_identity(child.pid),
                        'supervisor': os.getpid(), 'supervisor_start': child_identity(os.getpid())}
            write_at(state_fd, 'child.json', json.dumps(identity).encode('ascii'))
        emit('started', pid=child.pid)
        selector = selectors.DefaultSelector()
        for pipe, channel in [(child.stdout, 'stdout'), (child.stderr, 'stderr')]:
            os.set_blocking(pipe.fileno(), False)
            selector.register(pipe, selectors.EVENT_READ, channel)
        while selector.get_map():
            if marker('cancel') or time.monotonic() >= until:
                terminate_group()
                raise TimeoutError()
            for key, _ in selector.select(min(0.1, max(0, until - time.monotonic()))):
                chunk = os.read(key.fileobj.fileno(), 16384)
                if chunk:
                    emit(key.data, data=base64.b64encode(chunk).decode('ascii'))
                else:
                    selector.unregister(key.fileobj)
                    key.fileobj.close()
        pidfd = os.pidfd_open(child.pid, 0)
        try:
            waiter = selectors.DefaultSelector()
            waiter.register(pidfd, selectors.EVENT_READ)
            if not waiter.select(max(0, until - time.monotonic())):
                raise TimeoutError()
        finally:
            os.close(pidfd)
        with child_lock():
            code = child.wait(timeout=1)
            publish('ended')
        finished = True
        emit('exited', exit_code=code if code >= 0 else -code, exited=code >= 0)
        emit('success')
        wait_marker('ack', until)
    finally:
        if child is not None and not finished:
            terminate_group()
            with child_lock():
                child.wait(timeout=1)
        publish('finished')


def control():
    if action == 'release':
        publish('release')
    elif action == 'ack':
        publish('ack')
        wait_finished()
    elif action == 'terminate':
        publish('cancel')
        terminate_group()
        if marker('claimed'):
            wait_finished()
    elif action == 'clean':
        clean_attempt()
    else:
        raise ValueError()


def wait_finished():
    until = time.monotonic() + 1.5
    while not marker('finished'):
        if time.monotonic() >= until:
            raise TimeoutError()
        time.sleep(0.01)
