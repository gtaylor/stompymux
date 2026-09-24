import concurrent.futures,subprocess,os,pathlib,json,sys
root=pathlib.Path('/tmp/autopilot-ready');env=dict(os.environ,TMPDIR='/dev/shm/autopilot-acceptance')
label,exe=sys.argv[1:3];scenarios=['lateral','distant','retreat','reversals','circuit','short_occlusions','expiry','intercept_move']
def run(s):
 name=f'{label}-{s}';args=[exe,'--suite','pursuit','--scenario',s,'--fire','all','--seeds','3','--ticks','900','--trace',str(root/f'{name}.jsonl')]+(['--direct-pursuit'] if label.endswith('direct') else [])
 with (root/f'{name}.json').open('w') as out,(root/f'{name}.log').open('w') as err:subprocess.run(args,stdout=out,stderr=err,env=env,check=True)
 print(name,flush=True)
with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:list(pool.map(run,scenarios))
rows=[]
for s in scenarios:rows+=json.loads((root/f'{label}-{s}.json').read_text())
(root/f'{label}.json').write_text(json.dumps(rows,indent=2)+'\n')
