"""Fake account operations with real files, ownership, forks, and lock barriers."""

import codecs
import contextlib
import fcntl
import grp
import json
import os
import pwd
import select
import shutil
import signal
import stat
import subprocess
import sys
import traceback

HOME = '/home/user'
STATE = '/var/lib/sandbox-boat'
WORKSPACE = '/workspace'
CONTROLLER = 'fixture-controller'
CONTROLLER_ID = 10000
WORKLOAD_ID = 10001
IDS = ['1' * 32, '2' * 32, '3' * 32]


def wait_readable(fd):
    ready, _, _ = select.select([fd], [], [], 30)
    if not ready:
        raise TimeoutError('fixture actor did not reach its barrier')


class Actor:
    def __init__(self, pid, events, resume):
        self.pid, self.events, self.resume = pid, events, resume

    def event(self):
        wait_readable(self.events)
        return json.loads(os.read(self.events, 4096))

    def release(self):
        os.write(self.resume, b'1')

    def finish(self):
        while True:
            wait_readable(self.events)
            if not os.read(self.events, 4096):
                break
        _, status = os.waitpid(self.pid, 0)
        self.pid = None
        os.close(self.events)
        os.close(self.resume)
        return os.waitstatus_to_exitcode(status)

    def cancel(self):
        if self.pid is not None:
            os.kill(self.pid, signal.SIGKILL)
            os.waitpid(self.pid, 0)
            self.pid = None


class Accounts:
    def __init__(self, scenario, actor, events, resume):
        self.scenario, self.actor = scenario, actor
        self.events, self.resume = events, resume
        self.workload = 'sandbox-workload' if scenario == 'account' else 'fixture-workload'
        self.mkdir = os.mkdir
        self.flock = fcntl.flock

    def signal(self, phase, **data):
        os.write(self.events, json.dumps({'phase': phase, **data}).encode())

    def pause(self):
        wait_readable(self.resume)
        if os.read(self.resume, 1) != b'1':
            raise ValueError('fixture barrier closed')

    def account(self, name):
        if name == CONTROLLER:
            return pwd.struct_passwd((name, 'x', CONTROLLER_ID, CONTROLLER_ID, '', HOME, '/bin/sh'))
        with open('/accounts.json') as source:
            account = json.load(source)
        if name != self.workload or account['uid'] is None:
            self.signal('missing')
            self.pause()
            raise KeyError(name)
        uid = account['uid']
        return pwd.struct_passwd((name, 'x', uid, uid, '', WORKSPACE, '/usr/sbin/nologin'))

    def group(self, gid):
        return grp.struct_group((self.workload, 'x', gid, []))

    def run(self, args, **_kwargs):
        if args[0] == '/usr/sbin/useradd':
            with open('/accounts.json') as source:
                account = json.load(source)
            account['creations'] += 1
            account['uid'] = WORKLOAD_ID + account['creations'] - 1
            with open('/accounts.json', 'w') as target:
                json.dump(account, target)
            return subprocess.CompletedProcess(args, 0)
        if args[:4] != ['/usr/bin/sudo', '-n', '-l', '-U']:
            raise ValueError('unexpected account operation')
        denied = ('User ' + self.workload + ' is not allowed to run sudo on fixture\n').encode()
        return subprocess.CompletedProcess(args, 1, denied, b'')

    def make_directory(self, path, mode=0o777, *, dir_fd=None):
        if self.scenario == 'staging' and path == HOME + '/.sandbox-boat':
            if self.actor == 'second':
                self.signal('stage')
            result = self.mkdir(path, mode, dir_fd=dir_fd)
            if self.actor == 'first':
                self.signal('stage-created')
                self.pause()
            return result
        return self.mkdir(path, mode, dir_fd=dir_fd)

    def lock(self, fd, operation):
        if stat.S_ISREG(os.fstat(fd).st_mode):
            if self.actor == 'second':
                self.signal('lock')
            if self.actor == 'cancel':
                self.signal('lock', inode=os.fstat(fd).st_ino)
                self.pause()
        return self.flock(fd, operation)

    def install(self):
        pwd.getpwnam = self.account
        pwd.getpwall = lambda: [self.account(CONTROLLER), self.account(self.workload)]
        grp.getgrgid = self.group
        os.getgrouplist = lambda _name, gid: [gid]
        subprocess.run = self.run
        os.mkdir = self.make_directory
        fcntl.flock = self.lock


def launch(source, scenario, actor, attempt):
    events, output = os.pipe()
    incoming, resume = os.pipe()
    pid = os.fork()
    if pid == 0:
        os.close(events)
        os.close(resume)
        if scenario == 'abandon':
            sys.argv = ['abandon', attempt]
        else:
            accounts = Accounts(scenario, actor, output, incoming)
            accounts.install()
            sys.argv = ['bootstrap', HOME, STATE, WORKSPACE, CONTROLLER, accounts.workload, attempt]
        code = 0
        try:
            exec(compile(source, '<production-bootstrap>', 'exec'), {'__name__': '__main__'})
        except SystemExit as error:
            code = error.code
        except BaseException:
            traceback.print_exc()
            code = 98
        os._exit(code)
    os.close(output)
    os.close(incoming)
    return Actor(pid, events, resume)
