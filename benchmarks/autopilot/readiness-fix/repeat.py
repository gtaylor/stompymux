import subprocess,pathlib,os
r=pathlib.Path('/tmp/autopilot-ready');env=dict(os.environ,TMPDIR='/dev/shm/autopilot-acceptance')
for scenario in ['open','obstacles']:
 for label,exe in [('final','final-bench'),('before','reference-bench')]:
  name=f'pinned-{scenario}-{label}'
  with (r/f'{name}.csv').open('w') as out,(r/f'{name}.log').open('w') as err:
   subprocess.run(['taskset','-c','2,10',str(r/exe),'--warmup','35','--ticks','60','--repetitions','3','--scenario',scenario,'--fire','opportunistic'],stdout=out,stderr=err,env=env,check=True)
  print(name,flush=True)
