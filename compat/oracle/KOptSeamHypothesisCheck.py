# TEST-derived seam inputs; Apache-2.0, see compat/provenance.json.
import json,re,itertools
from pathlib import Path
source=Path('compat/oracle/KOptSeamDescriptorProbe.java').read_text()
records={r['case']:r for line in Path('work/round105-seam-probe.log').read_text().splitlines() if line.startswith('{') for r in [json.loads(line)]}
models=[]
for m in re.finditer(r'run\("([^"]+)",new int\[\]\{([^}]*)\},new int\[\]\{([^}]*)\},(\d+),([^;]+)\);',source):
 name,a,b,prefix,rest=m.groups();a=list(map(int,a.split(',')));b=list(map(int,b.split(',')));prefix=int(prefix)
 rem=list(map(int,re.search(r'(?:new int\[\]\{([^}]*)\})',rest).group(1).split(','))) if rest.startswith('new') else [6,1,2,3,4,5]
 flat=a[prefix:]+b;gaps=[len(a)-prefix-1,len(flat)-1]
 models.append((name,flat,gaps,a[:prefix],rem,records[name]['rows']))
# Original hypothesis: a program of cyclic endpoint reversals, transporting
# internal boundary gaps geometrically, acts consistently across all shapes.
def flip(a,gaps,u,v):
 i=a.index(u);j=a.index(v);n=len(a);length=(j-i)%n+1
 if length<=1 or length==n:return None
 indices=[(i+k)%n for k in range(length)];b=a[:]
 for p,x in zip(indices,[a[x] for x in indices][::-1]):b[p]=x
 new=[]
 for g in gaps:
  k=(g-i)%n
  new.append((i+length-2-k)%n if k<length-1 else g)
 anchor=b.index(a[0]);b=b[anchor:]+b[:anchor];new=[(g-anchor)%n for g in new]
 return b,new
def rows(a,gaps,prefix):
 g=sorted(gaps);return [prefix+a[:g[0]+1]+a[g[-1]+1:]]+[a[x+1:y+1] for x,y in zip(g,g[1:])]

virtual=[1,6,2,3,4,5];target=[1,5,2,6,4,3];endpoints=[6,1,2,3,4,5]
def flip_raw(a,gaps,i,j):
 n=len(a);length=(j-i)%n+1
 if length<=1 or length==n:return None
 indices=[(i+k)%n for k in range(length)];b=a[:]
 for pos,val in zip(indices,[a[x] for x in indices][::-1]):b[pos]=val
 return b,[(i+length-2-(g-i)%n)%n if (g-i)%n<length-1 else g for g in gaps]
def anchor(a,g):
 k=a.index(a[0])
 return a[k:]+a[:k],[(v-k)%len(a) for v in g]
hits=[];programs=[(virtual,[])]
for depth in range(1,5):
 nxt=[]
 for state,ops in programs:
  for u,v in itertools.permutations(range(6),2):
   result=flip_raw(state,[],state.index(endpoints[u]),state.index(endpoints[v]))
   if result is None:continue
   new=result[0];plan=ops+[(u,v)];k=new.index(1)
   if new[k:]+new[:k]==target:
    good=True
    for name,a,gaps,prefix,rem,want in models:
     a=a[:];gaps=gaps[:];positions=[a.index(x) for x in rem]
     for x,y in plan:
      result=flip_raw(a,gaps,positions[x],positions[y])
      if result is None:good=False;break
      a,gaps=result
     if not good:break
     k=a.index(rem[1]);a=a[k:]+a[:k];gaps=[(g-k)%len(a) for g in gaps]
     if rows(a,gaps,prefix)!=want:good=False;break
    if good:hits.append(plan)
   if depth<4:nxt.append((new,plan))
 programs=nxt
 if hits:break
print(json.dumps({'models':len(models),'matching_programs':hits[:10],'depth':depth,'hypothesis':'Precomputed original physical endpoint positions; virtual endpoint permutation tracks reversals; geometric interior-gap transport. No source-method credit.'},indent=2))
