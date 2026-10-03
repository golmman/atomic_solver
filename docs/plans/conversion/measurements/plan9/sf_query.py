import subprocess, sys, time, re

ENGINE='libs/Fairy-Stockfish/src/stockfish'

def query(fen, nodes, multipv=1):
    cmds = [
        'setoption name UCI_Variant value atomic',
        'setoption name Threads value 1',
        'setoption name Hash value 256',
        f'setoption name MultiPV value {multipv}',
        f'position fen {fen}',
        f'go nodes {nodes}',
    ]
    p = subprocess.Popen([ENGINE], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
    for c in cmds:
        p.stdin.write(c+'\n')
    p.stdin.flush()
    last = {}  # rank -> (depth, seldepth, score, nodes, pv)
    t0=time.time()
    best=None
    while True:
        line = p.stdout.readline()
        if not line: break
        line=line.strip()
        m = re.match(r'info depth (\d+) seldepth (\d+) multipv (\d+) score (cp|mate) (-?\d+) nodes (\d+).*\bpv (.+)$', line)
        if m:
            d,sd,k,sc,sv,n,pv = m.groups()
            last[int(k)] = dict(depth=int(d),seldepth=int(sd),score_kind=sc,score=int(sv),nodes=int(n),pv=pv.strip())
        if line.startswith('bestmove'):
            best=line; break
    wall=time.time()-t0
    p.stdin.write('quit\n'); p.stdin.flush(); p.wait()
    return last, wall, best

if __name__ == '__main__':
    fen = sys.argv[1]; nodes = int(sys.argv[2]); mpv = int(sys.argv[3]) if len(sys.argv)>3 else 1
    res, wall, best = query(fen, nodes, mpv)
    for k in sorted(res):
        r=res[k]
        print(f"multipv {k} depth {r['depth']}/{r['seldepth']} score {r['score_kind']} {r['score']} nodes {r['nodes']} wall {wall:.2f}s")
        print(f"  pv: {r['pv']}")
    print(best)
