import concurrent.futures,subprocess,os,pathlib,json
root=pathlib.Path('/tmp/autopilot-ready');env=dict(os.environ,TMPDIR='/dev/shm/autopilot-acceptance')
pathlib.Path(env['TMPDIR']).mkdir(exist_ok=True)
policies=['direct','control','d','e']
scenarios=['lateral','distant','retreat','reversals','circuit','short_occlusions','expiry','intercept_move']
def run(job):
 policy,scenario=job;name=f'screen-{policy}-{scenario}'
 args=[str(root/'candidate-encounters'),'--suite','pursuit','--scenario',scenario,'--fire','hold','--seeds','1','--ticks','900','--trace',str(root/f'{name}.jsonl')]
 args+=['--direct-pursuit'] if policy=='direct' else ['--pursuit-policy',policy]
 with (root/f'{name}.json').open('w') as out,(root/f'{name}.log').open('w') as err:
  subprocess.run(args,stdout=out,stderr=err,env=env,check=True)
 rows=json.loads((root/f'{name}.json').read_text())
 return dict(name=name,results=[dict(chassis=r['chassis'],first=r['first_ready'],outcome=r['outcome']) for r in rows])
with concurrent.futures.ThreadPoolExecutor(max_workers=8) as pool:
 for r in pool.map(run,[(p,s) for s in scenarios for p in policies]): print(json.dumps(r),flush=True)
for policy in policies:
 rows=[]
 for scenario in scenarios:rows+=json.loads((root/f'screen-{policy}-{scenario}.json').read_text())
 (root/f'screen-{policy}.json').write_text(json.dumps(rows,indent=2)+'\n')
