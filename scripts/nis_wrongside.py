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
SPREAD=0.28
for interval,bar_ms in (('1m',60_000),('5m',300_000)):
    t=pq.read_table(f'{DATA}/bars/XAUDUKA-{interval}.parquet')
    time=np.array([int(x.timestamp()*1000) for x in t.column('time').to_pylist()],dtype=np.int64)
    o=np.asarray(t.column('open').to_pylist(),float); h=np.asarray(t.column('high').to_pylist(),float)
    l=np.asarray(t.column('low').to_pylist(),float);  c=np.asarray(t.column('close').to_pylist(),float)
    tr=np.empty(len(c)); tr[0]=h[0]-l[0]
    tr[1:]=np.maximum(h[1:]-l[1:],np.maximum(np.abs(h[1:]-c[:-1]),np.abs(l[1:]-c[:-1])))
    atr=rma(tr,14)
    print(f'\n### XAUDUKA-{interval}')
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
                if i+1>=len(c) or time[i+1]-time[i]!=bar_ms: continue
                rng=h[j:i+1].max()-l[j:i+1].min(); imp=c[i]-o[j]
                if rng<=0 or imp==0: continue
                if abs(imp)<0.5*atr[j-1]: continue
                cand.append((c[i],rng,imp,o[i+1]))
            for f in (0.25,0.50,0.75,1.00):
                for mode,label in ((0,'brk'),(1,'rev')):
                    bad=0; tot=0
                    for (cl,rng,imp,nxt) in cand:
                        up = imp>0
                        long_ = up if mode==0 else (not up)
                        d=rng*f
                        stop = cl-d if long_ else cl+d
                        entry = nxt + (SPREAD/2 if long_ else -SPREAD/2)
                        tot+=1
                        if (long_ and stop>=entry) or ((not long_) and stop<=entry): bad+=1
                    print(f' {w} probe{probe} f={f:.2f} {label}: wrong-side {bad}/{tot}'
                          f' = {100*bad/max(tot,1):.1f}%')
