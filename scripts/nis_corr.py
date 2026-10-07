import re, statistics as st
from pathlib import Path
REC=Path('E:/rust/fd-news-inner-stop/receipts')
ROW=re.compile(r'^(nis\d+m-\w+-p\d-f\d+)\s+news-pulse\s+(\d+)\s+(-?[\d.]+)\s+(-?[\d.]+)')
def rows(name):
    d={}
    for line in (REC/f'{name}.txt').read_text(encoding='utf-8',errors='replace').splitlines():
        m=ROW.match(line)
        if m: d[m.group(1)]=(int(m.group(2)),float(m.group(3)),float(m.group(4)))
    return d
def corr(x,y):
    mx,my=st.mean(x),st.mean(y)
    num=sum((a-mx)*(b-my) for a,b in zip(x,y))
    den=(sum((a-mx)**2 for a in x)*sum((b-my)**2 for b in y))**0.5
    return num/den if den else float('nan')
for iv in ('1m','5m'):
    A=rows(f'nis_{iv}_A_noguards'); B=rows(f'nis_{iv}_B_noguards')
    k=sorted(set(A)&set(B))
    pfa=[A[r][1] for r in k]; pfb=[B[r][1] for r in k]
    ea=[A[r][2] for r in k]; eb=[B[r][2] for r in k]
    print(f'\n{iv}: {len(k)} rows   corr(PF_A,PF_B) = {corr(pfa,pfb):+.3f}'
          f'   corr(exp_A,exp_B) = {corr(ea,eb):+.3f}')
    print(f'  rows with PF>=1.200 on A only: {sum(1 for r in k if A[r][1]>=1.2 and B[r][1]<1.2)}'
          f' | B only: {sum(1 for r in k if B[r][1]>=1.2 and A[r][1]<1.2)}'
          f' | BOTH: {sum(1 for r in k if A[r][1]>=1.2 and B[r][1]>=1.2)}')
    print(f'  rows with exp>=+0.050R on A only: {sum(1 for r in k if A[r][2]>=0.05 and B[r][2]<0.05)}'
          f' | B only: {sum(1 for r in k if B[r][2]>=0.05 and A[r][2]<0.05)}'
          f' | BOTH: {sum(1 for r in k if A[r][2]>=0.05 and B[r][2]>=0.05)}')
    print(f'  sign of expectancy flips between windows on {sum(1 for r in k if ea[k.index(r)]*eb[k.index(r)]<0)} of {len(k)} rows')
    best_a=max(k,key=lambda r:A[r][1]); best_b=max(k,key=lambda r:B[r][1])
    print(f'  best on A: {best_a} PF {A[best_a][1]:.3f}/{A[best_a][2]:+.3f}R -> on B {B[best_a][1]:.3f}/{B[best_a][2]:+.3f}R')
    print(f'  best on B: {best_b} PF {B[best_b][1]:.3f}/{B[best_b][2]:+.3f}R -> on A {A[best_b][1]:.3f}/{A[best_b][2]:+.3f}R')
    # f-dependence per branch
    for tag in ('brk','rev'):
        for p in (1,2):
            ser=[(f,A.get(f'nis{iv}-{tag}-p{p}-f{f:03d}',(0,float("nan"),0))[1],
                     B.get(f'nis{iv}-{tag}-p{p}-f{f:03d}',(0,float("nan"),0))[1]) for f in (25,50,75,100)]
            print(f'   {tag} p{p}: A ' + ' '.join(f'{b:.3f}' for _,b,_ in ser)
                  + '   B ' + ' '.join(f'{c:.3f}' for _,_,c in ser) + '   (f=0.25/0.50/0.75/1.00)')
