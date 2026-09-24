import concurrent.futures,subprocess,os,pathlib,json,sys
root=pathlib.Path('/tmp/autopilot-ready');env=dict(os.environ,TMPDIR='/dev/shm/autopilot-acceptance');label,exe=sys.argv[1:3]
jobs=[('existing',None),('adversarial',None)]+[('adversarial',s) for s in ['late_clearance','alternating_clearance','waiting_controllers']]
def run(job):
 suite,scenario=job;name=label+'-'+(scenario or suite)
 args=[exe,'--suite',suite,'--ticks','240','--seeds','3','--trace',str(root/f'{name}.jsonl')]+(['--scenario',scenario] if scenario else [])
 with (root/f'{name}.json').open('w') as out,(root/f'{name}.log').open('w') as err:subprocess.run(args,stdout=out,stderr=err,env=env,check=True)
 print(name,flush=True)
with concurrent.futures.ThreadPoolExecutor(max_workers=5) as pool:list(pool.map(run,jobs))
