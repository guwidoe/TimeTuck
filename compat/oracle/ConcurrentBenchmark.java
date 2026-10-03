// Original measurement driver. Domain fixtures are attributed in ConcurrentOracle.
import java.io.*;
import java.util.*;
import java.util.stream.Collectors;
import ai.timefold.solver.core.preview.api.domain.metamodel.PlanningSolutionMetaModel;
import ai.timefold.solver.core.preview.api.move.builtin.Moves;
import ai.timefold.solver.core.preview.api.move.test.MoveTester;
public final class ConcurrentBenchmark {
    static Long[] state(ConcurrentOracle.Problem p) {
        var out=new ArrayList<Long>();
        for(var v:p.values) Collections.addAll(out,v.owner==null?null:(long)v.owner.id,v.index==null?null:(long)v.index,v.previous==null?null:(long)v.previous.id,v.next==null?null:(long)v.next.id,v.cascading,v.ready,v.start,v.finish,v.inconsistent?1L:0L);
        return out.toArray(Long[]::new);
    }
    static Long[] parse(String s) { return Arrays.stream(s.split(",")).map(v->v.equals("-")?null:Long.valueOf(v)).toArray(Long[]::new); }
    static String encode(Long[] s) { return Arrays.stream(s).map(v->v==null?"-":v.toString()).collect(Collectors.joining(",")); }
    static long checksum(Long[] s) { long sum=0;for(int i=0;i<s.length;i++)sum+=(i+1)*(s[i]==null?-1:s[i]);return sum; }
    static void check(Long[] a,Long[] b) { if(!Arrays.equals(a,b))throw new IllegalStateException("Shadow state mismatch: "+encode(a)+" != "+encode(b)); }
    public static void main(String[] args)throws Exception {
        int iterations=Integer.parseInt(args[0]),warmup=Integer.parseInt(args[1]),samples=Integer.parseInt(args[2]);
        if(iterations<1||iterations>10000||warmup<0||samples<1)throw new IllegalArgumentException("Counts");
        var meta=PlanningSolutionMetaModel.of(ConcurrentOracle.Problem.class,ConcurrentOracle.Owner.class,ConcurrentOracle.Value.class);
        var list=meta.genuineEntity(ConcurrentOracle.Owner.class).listVariable("values",ConcurrentOracle.Value.class);
        try(var input=new BufferedReader(new InputStreamReader(System.in))) {
            for(String line;(line=input.readLine())!=null;) {
                var f=line.split("\t",-1);if(f.length!=7)throw new IllegalArgumentException("Protocol");
                int n=Integer.parseInt(f[1]);if(n<2||n>64)throw new IllegalArgumentException("Size");
                var expectedInitial=parse(f[5]);var expectedTrial=parse(f[6]);
                if(expectedInitial.length!=9*n||expectedTrial.length!=9*n)throw new IllegalArgumentException("State size");
                for(int sample=-warmup;sample<samples;sample++) {
                    var p=ConcurrentOracle.create(n,f[2],f[3]);var context=MoveTester.build(meta).using(p);
                    check(state(p),expectedInitial);Long[][] trial={null};Long[] restored=null;long sum=0;
                    long start=System.nanoTime();
                    for(int i=0;i<iterations;i++) {
                        var op=f[4].split(",");if(op.length!=5)throw new IllegalArgumentException("Move");
                        var a=p.owners.get(Integer.parseInt(op[1]));int ai=Integer.parseInt(op[2]);
                        var b=p.owners.get(Integer.parseInt(op[3]));int bi=Integer.parseInt(op[4]);
                        var move=switch(op[0]) {case "c"->Moves.change(list,a,ai,b,bi);case "s"->Moves.swap(list,a,ai,b,bi);default->throw new IllegalArgumentException("Move");};
                        context.executeTemporarily(move,view->{trial[0]=state(p);check(trial[0],expectedTrial);});
                        restored=state(p);check(restored,expectedInitial);
                        for(var owner:p.owners)for(int j=0;j<owner.values.size();j++) {var v=owner.values.get(j);if(v.owner!=owner||v.index!=j)throw new IllegalStateException("Restored ownership");}
                        sum+=31*checksum(trial[0])+checksum(restored);
                    }
                    long elapsed=System.nanoTime()-start;
                    var op=f[4].split(",");var a=p.owners.get(Integer.parseInt(op[1]));int ai=Integer.parseInt(op[2]);var b=p.owners.get(Integer.parseInt(op[3]));int bi=Integer.parseInt(op[4]);
                    context.execute(op[0].equals("c")?Moves.change(list,a,ai,b,bi):Moves.swap(list,a,ai,b,bi));
                    var committed=state(p);check(committed,expectedTrial);
                    if(sample>=0)System.out.println(f[0]+"\t"+elapsed+"\t"+sum+"\t"+encode(trial[0])+"\t"+encode(restored)+"\t"+encode(committed));
                }
            }
        }
    }
}
