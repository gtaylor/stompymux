set pagination off
set confirm off
handle SIGPIPE nostop noprint pass
python
import gdb, threading, os, signal
for sample in range(80):
    timer = threading.Timer(10 if sample == 0 else 0.12, lambda: os.kill(os.getpid(), signal.SIGINT))
    timer.start()
    try:
        gdb.execute('run --scenario obstacles --fire opportunistic --warmup 35 --ticks 100 --repetitions 1' if sample == 0 else 'continue', to_string=True)
        gdb.execute('thread 1', to_string=True)
        print('SAMPLE', sample)
        print(gdb.execute('bt 30', to_string=True))
    except gdb.error as error:
        print(error)
        break
    finally:
        timer.cancel()
end
kill
quit
