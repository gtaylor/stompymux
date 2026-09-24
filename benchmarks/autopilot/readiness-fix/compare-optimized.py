import pathlib,hashlib,json,subprocess,sys,shutil
r=pathlib.Path('/tmp/autopilot-ready');d=pathlib.Path('benchmarks/autopilot/readiness-fix')
scenarios=['lateral','distant','retreat','reversals','circuit','short_occlusions','expiry','intercept_move','existing','adversarial','late_clearance','alternating_clearance','waiting_controllers']
results=[]
for reference in ['final','optimized-replay']:
 for s in scenarios:
  a=r/f'{reference}-{s}.jsonl';b=r/f'optimized-{s}.jsonl'
  def digest(p):
   h=hashlib.sha256()
   with p.open('rb') as f:
    for chunk in iter(lambda:f.read(1024*1024),b''):h.update(chunk)
   return h.hexdigest()
  ha,hb=digest(a),digest(b)
  results.append(dict(reference=reference,scenario=s,identical=ha==hb,reference_sha256=ha,candidate_sha256=hb,records=sum(1 for _ in b.open())))
(d/'optimized-trace-comparisons.json').write_text(json.dumps(results,indent=2)+'\n')
assert all(x['identical'] for x in results), results
for s in scenarios[8:]:
 base=pathlib.Path('benchmarks/autopilot/autopilot-clearance-artifacts')/f'{s}-candidate.json';tool='movement' if s=='existing' else 'adversarial'
 with (d/f'optimized-{s}-comparison.json').open('w') as out:subprocess.run([sys.executable,f'tools/compare_autopilot_{tool}.py',str(base),str(r/f'optimized-{s}.json')],stdout=out,check=True)
with (d/'optimized-pursuit-comparison.json').open('w') as out:subprocess.run([sys.executable,'tools/compare_autopilot_pursuit.py',str(r/'final-direct.json'),str(r/'optimized.json')],stdout=out,check=True)
for p in r.glob('optimized*.json'):shutil.copy2(p,d/p.name)
print('All 26 trace comparisons and behavior gates passed',flush=True)
