import argparse, csv, hashlib, json, os, pathlib, queue, random, shutil, sqlite3, statistics, subprocess, threading, time, tempfile
import psutil

ROOT = pathlib.Path(__file__).resolve().parent
p = argparse.ArgumentParser(); p.add_argument('--config', required=True); args=p.parse_args()
cfg=json.loads(pathlib.Path(args.config).read_text()); logs=ROOT/'logs'; logs.mkdir(exist_ok=True)
work=pathlib.Path(tempfile.gettempdir())/'tokensave-issue786-work'; work.mkdir(exist_ok=True)
rows=[]; checks=[]; seq=0
env=os.environ.copy(); env['TOKENSAVE_DISABLE_UPLOAD_COUNTER']='1'

def record(label, workload, rep, ms, cpu, rss, code, output):
    global seq
    seq+=1; name=f'{seq:05d}-{label}-{workload}-{rep}.json'
    (logs/name).write_text(json.dumps(output,ensure_ascii=False,indent=2),encoding='utf-8')
    rows.append(dict(binary=label,workload=workload,repetition=rep,elapsed_ms=ms,cpu_seconds=cpu,cpu_percent_one_core=cpu/(ms/1000)*100 if ms else 0,peak_rss_bytes=rss,exit_code=code,log='logs/'+name))
    with (ROOT/'benchmark-results.csv').open('w',newline='',encoding='utf-8') as f:
        w=csv.DictWriter(f,fieldnames=rows[0].keys()); w.writeheader(); w.writerows(rows)

def run(label, wd, argv, workload, rep, measured=True):
    start=time.perf_counter(); proc=subprocess.Popen([cfg[label],*argv],cwd=wd,env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
    out=[]
    t=threading.Thread(target=lambda:out.append(proc.communicate()),daemon=True);t.start()
    peak=0; cpu=0; ps=psutil.Process(proc.pid)
    while t.is_alive():
        try:
            peak=max(peak,ps.memory_info().rss); ct=ps.cpu_times(); cpu=ct.user+ct.system
        except psutil.Error: pass
        if time.perf_counter()-start>cfg['timeout_seconds']: proc.kill()
        t.join(.005)
    ms=(time.perf_counter()-start)*1000
    data={'command':argv,'stdout':out[0][0].decode('utf-8','replace'),'stderr':out[0][1].decode('utf-8','replace')}
    if measured: record(label,workload,rep,ms,cpu,peak,proc.returncode,data)
    if proc.returncode: raise RuntimeError(data)
    return data['stdout']

def check(name, passed, detail=''):
    checks.append(dict(name=name,passed=bool(passed),detail=detail))
    (ROOT/'correctness.json').write_text(json.dumps(checks,indent=2),encoding='utf-8')

def prepare():
    corpus=ROOT/'corpus'
    if corpus.exists() and (ROOT/'corpus-manifest.json').exists(): return corpus
    if corpus.exists(): shutil.rmtree(corpus)
    corpus.mkdir()
    paths=subprocess.check_output(['git','ls-tree','-r',cfg['commit']],cwd=cfg['repository']).decode().splitlines()
    manifest=[]
    for entry in paths:
        info,path=entry.split('\t',1); blob=info.split()[2]
        if pathlib.Path(path).suffix.lower() not in ('.c','.h','.py','.cpp','.hpp'): continue
        data=subprocess.check_output(['git','cat-file','blob',blob],cwd=cfg['repository'],stderr=subprocess.DEVNULL)
        short='sources/'+hashlib.sha256(path.encode()).hexdigest()[:12]+'/'+pathlib.Path(path).name
        target=corpus/short; target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes(data)
        manifest.append({'path':path,'corpus_path':short,'sha256':hashlib.sha256(data).hexdigest()})
    fixture=corpus/'validation_fixture'; fixture.mkdir()
    for i in range(80):
        (fixture/f'module_{i}.py').write_text(f'class Base{i}:\n    def execute(self, value):\n        return value + {i}\n\nclass Child{i}(Base{i}):\n    def calculate(self, value):\n        return self.execute(value)\n\ndef unique_function_{i}(value):\n    return Child{i}().calculate(value)\n',encoding='utf-8')
    (ROOT/'corpus-manifest.json').write_text(json.dumps(manifest,indent=2))
    return corpus

queries=[('fts_common','search',{'query':'can','limit':20}),('fts_rare','search',{'query':'unique_function_79','limit':20}),('fts_empty','search',{'query':'zzzz_no_such_symbol_786','limit':20}),('complexity','complexity',{'limit':20}),('complexity_scoped','complexity',{'path':'validation_fixture','limit':20}),('inheritance','inheritance_depth',{'limit':20}),('inheritance_scoped','inheritance_depth',{'path':'validation_fixture','limit':20}),('body','body',{'symbol':'unique_function_0','format':'json'}),('ambiguous','body',{'symbol':'execute','format':'json','limit':50}),('context','context',{'task':'CAN message processing','max_nodes':20,'include_code':True}),('hotspots','hotspots',{'limit':20}),('ambiguous_calls','ambiguous_calls',{'limit':200})]

class MCP:
    def __init__(self,label,wd):
        self.label=label;self.id=0;self.q=queue.Queue()
        self.err=(logs/f'mcp-{label}-stderr.txt').open('w',encoding='utf-8')
        self.p=subprocess.Popen([cfg[label],'serve','--path',str(wd),'--timings'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=self.err,env=env)
        self.ps=psutil.Process(self.p.pid)
        def read():
            for line in self.p.stdout:
                try:self.q.put(json.loads(line))
                except ValueError:pass
        threading.Thread(target=read,daemon=True).start()
        self.call('initialize',{'protocolVersion':'2024-11-05','capabilities':{},'clientInfo':{'name':'issue786-benchmark','version':'1'}})
        self.send({'jsonrpc':'2.0','method':'notifications/initialized'})
    def send(self,obj):self.p.stdin.write((json.dumps(obj)+'\n').encode());self.p.stdin.flush()
    def call(self,method,params):
        self.id+=1;self.send({'jsonrpc':'2.0','id':self.id,'method':method,'params':params})
        while True:
            result=self.q.get(timeout=cfg['timeout_seconds'])
            if result.get('id')==self.id:return result
    def query(self,name,tool,arguments,rep,measured):
        ct=self.ps.cpu_times(); c0=ct.user+ct.system; start=time.perf_counter()
        result=self.call('tools/call',{'name':'tokensave_'+tool,'arguments':arguments})
        ms=(time.perf_counter()-start)*1000;ct=self.ps.cpu_times()
        fail='error' in result or result.get('result',{}).get('isError',False)
        if measured:record(self.label,'mcp_'+name,rep,ms,ct.user+ct.system-c0,self.ps.memory_info().rss,int(fail),result)
        return result
    def close(self):
        self.p.stdin.close()
        try:self.p.wait(timeout=10)
        except subprocess.TimeoutExpired:self.p.kill();self.p.wait()
        self.err.close()

def normalize(x):
    if isinstance(x,dict):return {k:normalize(v) for k,v in x.items() if k not in ('id','_meta')}
    if isinstance(x,list):return [normalize(v) for v in x]
    if isinstance(x,str):
        for label in ('stable','beta'):x=x.replace(str(work/label),'<PROJECT>').replace(str(work/label).replace('\\','/'),'<PROJECT>')
    return x

def main():
    meta={k:{'path':cfg[k],'version':subprocess.check_output([cfg[k],'--version']).decode().strip(),'sha256':hashlib.sha256(pathlib.Path(cfg[k]).read_bytes()).hexdigest()} for k in ('stable','beta')}
    meta['date']=time.strftime('%Y-%m-%dT%H:%M:%S%z');meta['psutil']=psutil.__version__
    (ROOT/'binaries.json').write_text(json.dumps(meta,indent=2))
    corpus=prepare()
    for rep in range(cfg['index_repetitions']):
        for label in (('stable','beta') if rep%2==0 else ('beta','stable')):
            wd=work/label
            if wd.resolve().parent != work.resolve() or wd.name not in ('stable','beta'): raise RuntimeError('Unsafe workspace path')
            if wd.exists():shutil.rmtree(wd)
            shutil.copytree(corpus,wd)
            run(label,wd,['init',str(wd),'--no-git-hook','--cpu-usage',cfg['cpu_usage']],'initial_index',rep)
            if not (wd/'.tokensave'/'config.json').exists():raise RuntimeError('Index isolation failed')
            run(label,wd,['sync','--force'],'forced_index',rep)
            added=wd/'validation_fixture'/'added.py';added.write_text('def issue786_added():\n    return 786\n')
            run(label,wd,['sync'],'incremental_index',rep)
            found=run(label,wd,['tool','search','--query','issue786_added','--json'],'addition_check',rep)
            check(f'{label}-added-{rep}','issue786_added' in found and 'count: 1' in found)
            added.unlink();run(label,wd,['sync'],'deletion',rep)
            absent=run(label,wd,['tool','search','--query','issue786_added','--json'],'deletion_check',rep)
            check(f'{label}-deleted-{rep}','issue786_added' not in absent and 'count: 0' in absent)
            verify=run(label,wd,['verify'],'verify',rep);check(f'{label}-verify-{rep}',True,verify)
            sizes={str(f.relative_to(wd)):f.stat().st_size for f in (wd/'.tokensave').rglob('*.db')}
            record(label,'database_bytes',rep,sum(sizes.values()),0,0,0,sizes)
            print(label,'index round',rep,flush=True)
    for rep in range(-cfg['warmups'],cfg['cli_repetitions']):
        for name,tool,arguments in queries:
            outputs={}
            for label in (('stable','beta') if rep%2==0 else ('beta','stable')):
                outputs[label]=run(label,work/label,['tool',tool,'--args',json.dumps(arguments),'--json'],'cli_'+name,rep,rep>=0)
            if rep==0:
                try:eq=normalize(json.loads(outputs['stable']))==normalize(json.loads(outputs['beta']))
                except ValueError:eq=normalize(outputs['stable'])==normalize(outputs['beta'])
                check('cli-equivalence-'+name,eq,'Exact ordered output comparison; project roots normalized.')
        print('CLI round',rep,flush=True)
    servers={label:MCP(label,work/label) for label in ('stable','beta')}
    try:
        for rep in range(-cfg['warmups'],cfg['mcp_repetitions']):
            for name,tool,arguments in queries:
                outputs={}
                for label in (('stable','beta') if rep%2==0 else ('beta','stable')):
                    outputs[label]=servers[label].query(name,tool,arguments,rep,rep>=0)
                if rep==0:check('mcp-equivalence-'+name,normalize(outputs['stable'])==normalize(outputs['beta']),'Exact ordered output comparison; JSON-RPC IDs, timing metadata and roots normalized.')
            if rep%10==0:print('MCP round',rep,flush=True)
    finally:
        for server in servers.values():server.close()
    for label in ('stable','beta'):
        for db in (work/label/'.tokensave').rglob('*.db'):
            con=sqlite3.connect(f'file:{db.as_posix()}?mode=ro',uri=True)
            result=con.execute('pragma integrity_check').fetchall(); check(label+'-sqlite-integrity',result==[('ok',)],str(result))
            (logs/f'{label}-schema.json').write_text(json.dumps(con.execute("select type,name,sql from sqlite_master order by type,name").fetchall(),indent=2));con.close()
    print('COMPLETE',flush=True)

if __name__=='__main__': main()
