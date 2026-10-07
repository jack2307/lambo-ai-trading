import numpy as np, pyarrow.parquet as pq, datetime as dt
DATA='E:/rust/flowdesk/data'
def ms(s): return int(dt.datetime.fromisoformat(s).replace(tzinfo=dt.timezone.utc).timestamp()*1000)
WIN={"A'":(ms('2022-01-01'),ms('2026-01-01')),"B'":(ms('2018-01-01'),ms('2022-01-01'))}
ev=pq.read_table(f'{DATA}/news/events.parquet')
et=[int(x.timestamp()*1000) for x in ev.column('time').to_pylist()]
ec=ev.column('currency').to_pylist(); ei=ev.column('impact').to_pylist()
usd3=sorted(t for t,c,i in zip(et,ec,ei) if c and c.upper()=='USD' and i and i>=3)
def rma(x,n):
    out=np.full(len(x),np.nan); out[n-1]=np.mean(x[:n])
    for i in range(n,len(x)): out[i]=(out[i-1]*(n-1)+x[i])/n
    return out
def load(iv):
    t=pq.read_table(f'{DATA}/bars/XAUDUKA-{iv}.parquet')
    return (np.array([int(x.timestamp()*1000) for x in t.column('time').to_pylist()],dtype=np.int64),
            np.asarray(t.column('open').to_pylist(),float),np.asarray(t.column('high').to_pylist(),float),
            np.asarray(t.column('low').to_pylist(),float),np.asarray(t.column('close').to_pylist(),float))
B={iv:load(iv) for iv in ('1m','5m')}
SPREAD=0.28; RR=1.8; HOLD=14_400_000
def verdict(time,h,l,c,start_idx,long_,stop,target,t_end):
    # walk bars from start_idx while time < t_end; engine rule: stop first
    i=start_idx
    both=0
    while i<len(c) and time[i]<t_end:
        hit_s = l[i]<=stop if long_ else h[i]>=stop
        hit_t = h[i]>=target if long_ else l[i]<=target
        if hit_s and hit_t: return ('STOP',1,i)
        if hit_s: return ('STOP',0,i)
        if hit_t: return ('TARGET',0,i)
        i+=1
    return ('TIMEOUT',0,i)
def verdict_fine(time,h,l,c,t0,long_,stop,target,t_end):
    i=int(np.searchsorted(time,t0,'left'))
    while i<len(c) and time[i]<t_end:
        hit_s = l[i]<=stop if long_ else h[i]>=stop
        hit_t = h[i]>=target if long_ else l[i]<=target
        if hit_s and hit_t: return ('STOP',1)
        if hit_s: return ('STOP',0)
        if hit_t: return ('TARGET',0)
        i+=1
    return ('TIMEOUT',0)
for iv,bar_ms in (('1m',60_000),('5m',300_000)):
    time,o,h,l,c=B[iv]
    tr=np.empty(len(c)); tr[0]=h[0]-l[0]
    tr[1:]=np.maximum(h[1:]-l[1:],np.maximum(np.abs(h[1:]-c[:-1]),np.abs(l[1:]-c[:-1])))
    atr=rma(tr,14)
    f1t,f1o,f1h,f1l,f1c=B['1m']
    print(f'\n### XAUDUKA-{iv}  (both-touched on own bars; {iv} verdict vs 1m verdict)')
    for w,(a,b) in WIN.items():
        for probe in (1,2):
            cand=[]
            for e in usd3:
                if not (a<=e<b): continue
                j=int(np.searchsorted(time,e,'left'))
                if j<=0 or j+probe>=len(c): continue
                if time[j-1]>=e or time[j]-e>15*60_000: continue
                if not np.isfinite(atr[j-1]) or atr[j-1]<=0: continue
                i=j+probe-1
                if any(time[k+1]-time[k]!=bar_ms for k in range(j,i)): continue
                if i+1>=len(c): continue
                rng=h[j:i+1].max()-l[j:i+1].min(); imp=c[i]-o[j]
                if rng<=0 or imp==0 or abs(imp)<0.5*atr[j-1]: continue
                cand.append((i,c[i],rng,imp))
            for f in (0.25,0.50,1.00):
                for mode,label in ((0,'brk'),(1,'rev')):
                    n=0; both=0; dis=0; mix={'STOP':0,'TARGET':0,'TIMEOUT':0}
                    for (i,cl,rng,imp) in cand:
                        up=imp>0; long_= up if mode==0 else (not up)
                        d=rng*f
                        stop = cl-d if long_ else cl+d
                        entry = o[i+1] + (SPREAD/2 if long_ else -SPREAD/2)
                        risk = abs(entry-stop)
                        target = entry + risk*RR if long_ else entry - risk*RR
                        t_end = time[i+1]+HOLD
                        v,bt,_ = verdict(time,h,l,c,i+1,long_,stop,target,t_end)
                        vf,_ = verdict_fine(f1t,f1h,f1l,f1c,time[i+1],long_,stop,target,t_end)
                        n+=1; both+=bt; mix[v]+=1
                        if v!=vf: dis+=1
                    if n:
                        print(f' {w} p{probe} f={f:.2f} {label}: n={n} both-touched {100*both/n:5.1f}%'
                              f'  disagree-vs-1m {100*dis/n:5.1f}%  exits S/T/TO {mix["STOP"]}/{mix["TARGET"]}/{mix["TIMEOUT"]}')
