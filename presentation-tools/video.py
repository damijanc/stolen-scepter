import sys
src=[l for l in open('master.txt').read().split('\n') if l]
WALK,FIGHT=int(sys.argv[1]),int(sys.argv[2])
out=[];n=[0];step=2;combat=False;skip=0
def shot():
    out.append(f'shot f{n[0]:05d}'); n[0]+=1
for l in src:
    if l.startswith('#SHOT'):
        name=l.split()[1]
        if name in('battle1-start','siege-start') or name.startswith('fight') and name.endswith('-start'): combat=True; step=FIGHT
        elif name in('battle1-victory','siege-end') or (name.startswith('walk')): 
            combat = combat and not name.startswith('battle1-victory') and name!='siege-end'
            if name.startswith('walk'): combat=False
            step=WALK if name.startswith('walk') else 2
        elif name in ('town-menu','siege-prompt','chest-choice','before-chest'): step=2
        continue
    if l.startswith('#HOLD'):
        if not combat:
            h=int(l.split()[1])
            for _ in range(h//2): out.append('wait 2'); shot()
        continue
    if l.startswith('wait'):
        v=l.split()[1]; f=int(round(float(v[:-1])*60)) if v.endswith('s') else int(v)
        while f>0:
            s=min(step,f); out.append(f'wait {s}'); f-=s
            if s==step or not combat: shot()
        continue
    out.append(l)
    if not combat or step<=2: shot()
open('video-script.txt','w').write('\n'.join(out)+'\n')
print(n[0],'frames',n[0]/30,'s')
