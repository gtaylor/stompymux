#!/usr/bin/env python3
"""Optional admission boundary comparison; all databases and sockets are temporary."""
import json
import pathlib
import shutil
import signal
import socket
import sqlite3
import subprocess
import tempfile
import time
from behavioral_probe import Client, ROOT, HASH


def send_until(client, value, expected):
    """Allow bounded password work to finish instead of sampling only its first quiet gap."""
    output = client.send(value)
    deadline = time.monotonic() + 10
    while expected not in output and time.monotonic() < deadline:
        output += client.read()
    return output


def probe(engine):
    with tempfile.TemporaryDirectory(prefix='mux-admission-audit-') as directory:
        game=pathlib.Path(directory)
        shutil.copytree(ROOT/'tests/fixtures/game',game,dirs_exist_ok=True)
        with sqlite3.connect(game/'data/stompymux.db') as db:
            db.execute('UPDATE player_state SET password_hash=?',[HASH])
            db.execute('UPDATE objects SET has_wizard_flag=0 WHERE dbref=2')
            db.execute('UPDATE player_state SET failed_login_count=failed_login_count+3,unreported_failed_login_count=3 WHERE object_dbref=1')
            db.execute("INSERT OR REPLACE INTO player_login_history VALUES(1,1,0,1700000000,'audit-failure-host')")
        with socket.socket() as sock:
            sock.bind(('127.0.0.1',0)); port=sock.getsockname()[1]
        path=game/'stompymux.toml'
        path.write_text(path.read_text().replace('port = 5555',f'port = {port}'))
        binary=ROOT.parent/'btmux-khi/build/stompymux' if engine=='c' else ROOT/'target/debug/stompymux-rs'
        args=[str(binary),'stompymux.toml'] if engine=='c' else [str(binary),'serve','--game-dir',str(game)]
        clients=[]
        with (game/'process.log').open('w+') as log:
            process=subprocess.Popen(args,cwd=game,stdout=log,stderr=log)
            try:
                for _ in range(200):
                    try: admin=Client(port); break
                    except OSError:
                        if process.poll() is not None: log.seek(0); raise RuntimeError(log.read())
                        time.sleep(.05)
                else: raise RuntimeError('startup timeout')
                clients.append(admin); admin.read(); admin.send('#1')
                output=[{'step':'god login','output':admin.send('auditpass')},
                        {'step':'limit','output':admin.send('@admin max_players=1')}]
                signup=Client(port); clients.append(signup); signup.read()
                for step,value in [('name','AuditRegistrant'),('confirm','yes'),('password','auditpass')]:
                    output.append({'step':step,'output':signup.send(value)})
                output.append({'step':'retype','output':send_until(signup,'auditpass','Connected.')})
                admin.read()
                output.append({'step':'registered','output':admin.send('@find AuditRegistrant')})
                existing=Client(port); clients.append(existing); existing.read()
                output.append({'step':'existing name','output':existing.send('#2')})
                output.append({'step':'existing password','output':existing.send('auditpass')})
                return output
            finally:
                for client in clients: client.socket.close()
                if process.poll() is None:
                    process.send_signal(signal.SIGTERM)
                    try: process.wait(timeout=10)
                    except subprocess.TimeoutExpired: process.kill(); process.wait()

if __name__=='__main__':
    print(json.dumps({engine:probe(engine) for engine in ['c','rust']},indent=2))
