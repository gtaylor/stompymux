"""Run only after builds, tests and encounter jobs have finished."""
import subprocess,pathlib,os
root=pathlib.Path('/tmp/autopilot-acceptance');env=dict(os.environ,TMPDIR='/dev/shm/autopilot-acceptance')
for name,exe,extra in [('cpu-before','behavior-bench',[]),('cpu-final','final-bench',[]),('moving-before','behavior-bench',['--scenario','moving_pursuit']),('moving-final','final-bench',['--scenario','moving_pursuit'])]:
 with (root/f'{name}.csv').open('w') as out,(root/f'{name}.log').open('w') as err:
  subprocess.run([str(root/exe),'--warmup','35','--ticks','60','--repetitions','3']+extra,stdout=out,stderr=err,env=env,check=True)
 print(name,flush=True)
