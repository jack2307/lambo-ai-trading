import math
def Phi(z): return 0.5*(1.0+math.erf(z/math.sqrt(2.0)))
def power_one(theta1,p50,p95,alpha_z=1.645):
    sig=(p95-p50)/1.645
    return Phi((theta1-p50)/sig - alpha_z), sig
def mde(p50,p95,beta_z=0.8416,alpha_z=1.645):
    sig=(p95-p50)/1.645
    return p50+(alpha_z+beta_z)*sig, sig

print("M2 - power of the SECOND window to detect the FIRST window's own point estimate")
print("one-sided alpha = 0.05 against the second window's own published null (profit-factor units)\n")
rows=[
 # id, theta1 (first window PF), second window PF, null p50, null p95, n2, source
 ("fri   gold weekend hold",      2.064, 0.983, 0.799, 1.022, 400, "runs/2026-09-13-friday-weekend-hold/in-sample-fixed.txt"),
 ("eu    EURUSD 03:00-11:00 NY",  1.069, 0.926, 0.895, 0.984, 2057,"runs/2026-09-14-fx-local-hours-sign/in-sample-fixed.txt"),
 ("eu    same, direction null",   1.069, 0.926, 0.902, 1.003, 2057,"decision 2026-09-14-fx-local-hours-sign.md"),
 ("xag20 silver 20-day sign",     1.341, 1.026, 0.949, 1.233, 186, "runs/2026-09-13-tsmom-silver/out-of-sample-fixed.txt"),
 ("close gold 16:30-18:15 NY",    1.254, 1.299, 0.892, 1.262, 282, "runs/2026-09-13-close-reopen-drift/out-of-sample-fixed.txt"),
]
for name,t1,pf2,p50,p95,n2,src in rows:
    pw,sig=power_one(t1,p50,p95)
    m,_=mde(p50,p95)
    z2=(pf2-p50)/sig
    print("%-30s n2=%5d  sigma_null %.4f PF" % (name,n2,sig))
    print("    first window point estimate PF %.3f -> power of window 2 = %.3f" % (t1,pw))
    print("    window 2 observed PF %.3f -> z = %+.2f (its own null)" % (pf2,z2))
    print("    smallest PF window 2 could detect at 0.80 power: %.3f (null median %.3f)" % (m,p50))
    print("    source: %s" % src)
    print()

print("\nM2b - eu in the units its own registration declared: pips of detrended excess per hold")
print("published per-hold standard deviations 40.8 (window 1) and 27.5 (window 2) pips\n")
for nm,n,sd,obs in (("window 1  2010-06 -> 2018-06",2088,40.8,-2.84),("window 2  2018-06 -> 2026-05",2056,27.5,-0.10)):
    se=sd/math.sqrt(n)
    print("%s  n=%d  SD %.1f pips  SE %.4f pips  observed %+.2f pips -> t = %+.2f"%(nm,n,sd,se,obs,obs/se))
se2=27.5/math.sqrt(2056)
for true in (-2.84,-1.5,-1.0):
    ncp=abs(true)/se2
    pw=Phi(ncp-1.96)+Phi(-ncp-1.96)
    print("  window 2 power to detect a TRUE excess of %+.2f pips/hold (two-sided 0.05): %.3f  (t would be %+.2f)"%(true,pw,true/se2))
print("  smallest excess window 2 could detect at 0.80 power (two-sided 0.05): %+.3f pips/hold"%(-(1.96+0.8416)*se2))
print("  window 1 SE %.4f pips -> its own t %+.2f, two-sided p %.5f"%(40.8/math.sqrt(2088),-2.84/(40.8/math.sqrt(2088)),2*(1-Phi(2.84/(40.8/math.sqrt(2088))))))
