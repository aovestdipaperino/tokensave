"""Additional bounded stability checks; run after benchmark.py, before summarize.py."""
import concurrent.futures, csv, json, pathlib, shutil, subprocess, time
import benchmark as b
b.rows=list(csv.DictReader((b.ROOT/'benchmark-results.csv').open(encoding='utf-8')))
b.seq=max(int(p.name.split('-')[0]) for p in b.logs.glob('[0-9]*.json'))
b.checks=json.loads((b.ROOT/'correctness.json').read_text())
details={}
for label in ('stable','beta'):
    server=b.MCP(label,b.work/label);start=time.perf_counter();n=0;errors=0;samples=[]
    try:
        while time.perf_counter()-start<60:
            name,tool,args=b.queries[n%len(b.queries)]
            result=server.query('soak',tool,args,n,False)
            errors+=int('error' in result or result.get('result',{}).get('isError',False));n+=1
            if n%100==0:samples.append({'seconds':time.perf_counter()-start,'rss':server.ps.memory_info().rss,'requests':n})
    finally:server.close()
    details[label]={'soak_seconds':time.perf_counter()-start,'requests':n,'errors':errors,'rss_samples':samples}
    b.check(label+'-60s-soak-no-errors',errors==0,f'{n} serial requests; bounded smoke soak, not production duration.')
    print(label,'soak complete',n,flush=True)
    def client(i):
        s=b.MCP(label,b.work/label);errors=[]
        try:
            for j in range(30):
                name,tool,args=b.queries[j%len(b.queries)]
                r=s.query(name,tool,args,j,False)
                if 'error' in r or r.get('result',{}).get('isError',False):errors.append(r)
        finally:s.close()
        return errors
    with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:
        results=list(pool.map(client,range(4)))
    details[label]['concurrent_errors']=results
    b.check(label+'-four-reader-processes',not any(results),'Four MCP processes, 30 requests each, same database; no concurrent writer.')
    wd=b.work/(label+'-cancel')
    if wd.resolve().parent != b.work.resolve() or wd.name not in ('stable-cancel','beta-cancel'):raise RuntimeError('Unsafe workspace path')
    if wd.exists():shutil.rmtree(wd)
    shutil.copytree(b.work/label,wd)
    with (b.logs/(label+'-cancel-output.txt')).open('w',encoding='utf-8') as log:
        proc=subprocess.Popen([b.cfg[label],'sync','--force'],cwd=wd,stdout=log,stderr=log)
        time.sleep(.05);terminated=proc.poll() is None
        if terminated:proc.terminate()
        proc.wait(timeout=30)
    details[label]['cancellation']={'terminated_running_process':terminated,'exit_code':proc.returncode,'delay_seconds':.05,'write_phase_confirmed':False}
    try:
        recovery=b.run(label,wd,['sync','--force'],'cancellation_recovery',0)
        verify=b.run(label,wd,['verify'],'cancellation_verify',0)
        b.check(label+'-forced-termination-recovery',True,verify+'Write phase at termination unconfirmed; not graceful MCP cancellation.')
    except Exception as exc:b.check(label+'-forced-termination-recovery',False,str(exc))
(b.ROOT/'extended-results.json').write_text(json.dumps(details,indent=2))
print('Extended checks complete',flush=True)
