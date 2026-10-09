"""Virtual-best-solver (VBS) portfolio estimate over the lean plan10 history/killer arms.

Reads the committed per-case quick-suite results under
docs/plans/lean/measurements/plan10/results/ (59 cases x 24 configs, first-outcome
child_evals) and reports, per portfolio size K, the ratio of sum-of-per-case-min
child_evals to the default config's total. Run from the repo root. Read-only.
"""
import sys
sys.dont_write_bytecode = True
import json,glob,itertools,math,random,statistics as st
R='docs/plans/lean/measurements/plan10/results/'
def load(p):
    d=json.load(open(p)); return {r['name']:(r['child_evals'],r['status']=='ok' and not r['wrong']) for r in d['results']}
base=load(R+'quick_baseline.json')
arms={p.split('quick_arm_')[1][:-5]:load(p) for p in glob.glob(R+'quick_arm_*.json')}
# drop extreme probes (k0 = killer off, hb1 = history bonus 1) only as sensitivity
cases=sorted(base)
print('cases',len(cases),'arms',len(arms), sorted(arms))
def ev(cfg,c): return (base if cfg=='base' else arms[cfg])[c]
def report(names,label):
    tot_b=sum(base[c][0] for c in cases)
    tot_m=0; ratios=[]; unsolved=0
    for c in cases:
        vals=[ev(n,c) for n in names]
        ok=[v for v,o in vals if o]
        m=min(ok) if ok else min(v for v,_ in vals)
        if not ok: unsolved+=1
        tot_m+=m; ratios.append(m/base[c][0])
    gm=math.exp(sum(map(math.log,ratios))/len(ratios))
    big=sorted(cases,key=lambda c:-base[c][0])[:10]
    tb=sum(base[c][0] for c in big); tm=sum(min(v for v,o in [ev(n,c) for n in names] if o) for c in big)
    print(f'{label:40s} K={len(names):2d} total {tot_m/tot_b:.3f}  geomean {gm:.3f}  top10-hardest total {tm/tb:.3f}  unsolved {unsolved}')
allarms=['base']+sorted(arms)
report(allarms,'VBS all 24 configs')
report(['base']+[a for a in sorted(arms) if a not in('k0','hb1')],'VBS excl k0/hb1')
random.seed(1)
others=[a for a in sorted(arms)]
for K in (2,3,4,8):
    tots=[];gms=[];tops=[]
    combos=list(itertools.combinations(others,K-1))
    random.shuffle(combos)
    for comb in combos[:400]:
        names=['base',*comb]
        tb=sum(base[c][0] for c in cases); tm=0; rs=[]
        for c in cases:
            ok=[v for v,o in (ev(n,c) for n in names) if o]
            m=min(ok); tm+=m; rs.append(m/base[c][0])
        big=sorted(cases,key=lambda c:-base[c][0])[:10]
        tops.append(sum(min(v for v,o in (ev(n,c) for n in names) if o) for c in big)/sum(base[c][0] for c in big))
        tots.append(tm/tb); gms.append(math.exp(sum(map(math.log,rs))/len(rs)))
    print(f'random base+{K-1} arms (n={len(tots)}): total median {st.median(tots):.3f} [min {min(tots):.3f} max {max(tots):.3f}]  geomean median {st.median(gms):.3f}  top10 median {st.median(tops):.3f}')
# hardest cases detail for one plausible a-priori portfolio
names=['base','hs2','ag100','cr200']
print('\ncase  base  min-of',names)
for c in sorted(cases,key=lambda c:-base[c][0])[:12]:
    vals=[ev(n,c)[0] for n in names]
    allv=[ev(n,c) for n in allarms]
    print(f'{c:12s} {base[c][0]:>10d}  p4min {min(vals)/base[c][0]:.2f}  vbs {min(v for v,o in allv if o)/base[c][0]:.2f}  max {max(v for v,o in allv)/base[c][0]:.2f}')
