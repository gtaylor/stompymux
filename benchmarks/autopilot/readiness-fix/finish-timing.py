"""Run only after builds, tests and encounter jobs have finished."""
import subprocess,pathlib,os,csv
root=pathlib.Path('/tmp/autopilot-ready');env=dict(os.environ,TMPDIR='/dev/shm/autopilot-acceptance')
for name,exe,extra in [('cpu-geometry-reference','final-bench',[]),('cpu-optimized','optimized-bench',[]),('moving-geometry-reference','final-bench',['--scenario','moving_pursuit']),('moving-optimized','optimized-bench',['--scenario','moving_pursuit'])]:
 with (root/f'{name}.csv').open('w') as out,(root/f'{name}.log').open('w') as err:
  subprocess.run([str(root/exe),'--warmup','35','--ticks','60','--repetitions','3']+extra,stdout=out,stderr=err,env=env,check=True)
 if name.endswith('optimized'):
  for row in csv.DictReader((root/f'{name}.csv').open()):
   limit=75 if row['fire']=='true' else 50
   assert float(row['p95_autopilot_ms']) < limit, (name,row['scenario'],row['fire'],row['p95_autopilot_ms'],limit)
 print(name,flush=True)
