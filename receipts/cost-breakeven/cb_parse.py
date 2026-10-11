import re, sys, json

HDR = re.compile(r'^(\S+)\s+(\S+)\s+(\d+)\s+(-?[\d.]+)\s+(-?[\d.]+)\s+(-?[\d.]+)\s+(-?[\d.]+)\s+(-?[\d.]+)\s+(-?\d+)%\s+(.*)$')
LBAR = re.compile(r'Lbar ([\-\d.]+) R = \|avg_loss_r\| ([\-\d.]+) x loss share ([\-\d.]+) over (\d+) trades; PF_r ([\-\d.]+) \(R\) vs PF_usd ([\-\d.]+) \(USD, gap ([+\-][\d.]+)\); identity E = Lbar\(PF_r-1\) = ([+\-][\d.]+) R vs expectancy ([+\-][\d.]+) R, residual ([+\-][\d.]+) R; expectancy leg (\w+)')
DD   = re.compile(r'max drawdown ([\-\d.]+) USD = ([\-\d.]+)% of the book.s peak equity.*?avg_mae ([\-\d.]+) R is\); net ([+\-][\d.]+) USD over (\d+) trades')
GRD  = re.compile(r'guards: (.*?); sized down (\d+)')
HOLD = re.compile(r'mean hold ([\d.]+) min')

def parse(path):
    rows={}; cur=None
    for line in open(path, encoding='utf-8', errors='replace'):
        line=line.rstrip('\n')
        m=HDR.match(line)
        if m and not line.startswith(' '):
            lab=m.group(1)
            cur={'label':lab,'base':m.group(2),'n':int(m.group(3)),
                 'pf_usd_col':float(m.group(4)),'expect_col':float(m.group(5)),
                 'null_p50':float(m.group(6)),'null_p95':float(m.group(7)),
                 'swap':float(m.group(8)),'pct':int(m.group(9)),'verdict':m.group(10)}
            rows[lab]=cur
            continue
        if cur is None: continue
        s=line.strip()
        m=LBAR.search(s)
        if m:
            cur.update(Lbar=float(m.group(1)), avg_loss_r=float(m.group(2)), loss_share=float(m.group(3)),
                       n_lbar=int(m.group(4)), pf_r=float(m.group(5)), pf_usd=float(m.group(6)),
                       gap=float(m.group(7)), E=float(m.group(8)), expect=float(m.group(9)),
                       resid=float(m.group(10)), leg=m.group(11))
        m=DD.search(s)
        if m:
            cur.update(dd_usd=float(m.group(1)), dd_pct=float(m.group(2)), avg_mae=float(m.group(3)), net_usd=float(m.group(4)))
        m=GRD.search(s)
        if m:
            cur.update(guard_txt=m.group(1), sized_down=int(m.group(2)))
        elif s.startswith('guards: nothing acted'):
            # hypotheses.rs:747 prints this INSTEAD of the three-part line when
            # skipped/closed are empty AND sized_down_by_guard == 0, so this 0
            # is read from the code path, not from a missing string.
            cur.update(guard_txt='nothing acted', sized_down=0)
        m=HOLD.search(s)
        if m: cur.update(mean_hold=float(m.group(1)))
        if s.startswith('exits:'):
            cur['exits']=s[6:].strip()[:4000]
    return rows

if __name__=='__main__':
    out={}
    for p in sys.argv[1:]:
        out[p]=parse(p)
    json.dump(out, open(sys.argv[1]+'.json','w'))
    for p,r in out.items():
        print(p, len(r), 'rows')
