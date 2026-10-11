import sys; sys.path.insert(0,r'C:/Users/Admin/AppData/Local/Temp/claude/e--nodejs-backcom-vantage/a5c7fd65-9276-457f-850d-25cb44e74c74/scratchpad/cb')
from cb_parse import parse
S=r'C:/Users/Admin/AppData/Local/Temp/claude/e--nodejs-backcom-vantage/a5c7fd65-9276-457f-850d-25cb44e74c74/scratchpad/cb/'
IS=parse(S+'C-is-s28.txt'); OOS=parse(S+'C-oos-s28.txt')
labs=sorted(set(IS)&set(OOS))
sel=[]
for L in labs:
    a,b=IS[L],OOS[L]
    if 'pf_r' not in a or 'pf_r' not in b: continue
    if a['n']<40 or b['n']<40: continue
    m=min(a['pf_r'],b['pf_r'])
    if 1.05<=m<1.20: sel.append((m,L,a,b))
sel.sort(reverse=True)
print('CRITERION: min(PF_r over the two windows) in [1.05,1.20) AND n>=40 on BOTH, guards ON, spread 0.28')
print('hits:',len(sel),'of',len([L for L in labs if 'pf_r' in IS[L] and 'pf_r' in OOS[L] and IS[L]['n']>=40 and OOS[L]['n']>=40]),'readable row-arms with n>=40 both')
print(f"{'label':14s} {'base':16s} {'minPFr':>7s} {'n_is':>5s} {'n_oos':>5s} {'PFr_is':>7s} {'PFr_oos':>7s} {'E_is':>8s} {'E_oos':>8s} {'Lbar_is':>7s} {'Lbar_o':>7s} {'sd':>4s} {'ddUSD_is':>9s} {'ddUSD_o':>9s} {'dd%_o':>6s} {'leg':>4s}")
for m,L,a,b in sel:
    print(f"{L:14s} {a['base']:16s} {m:7.4f} {a['n']:5d} {b['n']:5d} {a['pf_r']:7.4f} {b['pf_r']:7.4f} {a['E']:+8.4f} {b['E']:+8.4f} {a['Lbar']:7.4f} {b['Lbar']:7.4f} {a.get('sized_down',-1):4d} {a.get('dd_usd',0):9.2f} {b.get('dd_usd',0):9.2f} {b.get('dd_pct',0):6.2f} {a.get('leg','?')[:4]:>4s}")
