import pathlib,json,hashlib,sys,subprocess,collections
r=pathlib.Path('/tmp/autopilot-acceptance');d=pathlib.Path('benchmarks/autopilot/acceptance-followup')
scenarios=['lateral','distant','retreat','reversals','circuit','short_occlusions','expiry','intercept_move'];results=[]
for prefix in ['before','replay']:
 for s in scenarios:
  a=r/f'{prefix}-{s}.jsonl';b=r/f'final-{s}.jsonl'
  ha=hashlib.sha256(a.read_bytes()).hexdigest();hb=hashlib.sha256(b.read_bytes()).hexdigest()
  results.append(dict(comparison=prefix,scenario=s,identical=ha==hb,reference_sha256=ha,candidate_sha256=hb,records=sum(1 for _ in b.open())))
for s in ['existing','adversarial','late_clearance','alternating_clearance','waiting_controllers']:
 a=r/f'replay-{s}.jsonl';b=r/f'final-{s}.jsonl';ha=hashlib.sha256(a.read_bytes()).hexdigest();hb=hashlib.sha256(b.read_bytes()).hexdigest()
 results.append(dict(comparison='regression_replay',scenario=s,identical=ha==hb,reference_sha256=ha,candidate_sha256=hb,records=sum(1 for _ in b.open())))
(d/'trace-comparisons.json').write_text(json.dumps(results,indent=2)+'\n');print('trace matches',sum(x['identical'] for x in results),'/',len(results))
for s in ['existing','adversarial','late_clearance','alternating_clearance','waiting_controllers']:
 base=pathlib.Path('benchmarks/autopilot/autopilot-clearance-artifacts')/f'{s}-candidate.json';tool='movement' if s=='existing' else 'adversarial'
 with (d/f'{s}-comparison.json').open('w') as out:subprocess.run([sys.executable,f'tools/compare_autopilot_{tool}.py',str(base),str(r/f'final-{s}.json')],stdout=out,check=False)
with (d/'pursuit-comparison.json').open('w') as out:subprocess.run([sys.executable,'tools/compare_autopilot_pursuit.py',str(r/'final-direct.json'),str(r/'final.json')],stdout=out,check=False)
for p in r.glob('final*.json'):
 (d/p.name).write_bytes(p.read_bytes())
