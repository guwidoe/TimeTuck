// Original adapter: generated explicit-group transactions, public Community APIs only.
import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.util.*;
import java.util.stream.Collectors;
import ai.timefold.solver.core.api.domain.entity.PlanningEntity;
import ai.timefold.solver.core.api.domain.solution.*;
import ai.timefold.solver.core.api.domain.valuerange.ValueRangeProvider;
import ai.timefold.solver.core.api.domain.variable.PlanningVariable;
import ai.timefold.solver.core.api.score.SimpleScore;
import ai.timefold.solver.core.preview.api.domain.metamodel.*;
import ai.timefold.solver.core.preview.api.move.Move;
import ai.timefold.solver.core.preview.api.move.builtin.Moves;
import ai.timefold.solver.core.preview.api.move.test.MoveTester;
import ai.timefold.solver.core.preview.api.neighborhood.stream.dataset.sample.Sample;

public final class PillarMoveBenchmark {
    @PlanningEntity public static class Entity {
        @PlanningVariable(valueRangeProviderRefs="values", allowsUnassigned=true) public Integer a;
        @PlanningVariable(valueRangeProviderRefs="values", allowsUnassigned=true) public Integer b;
        @PlanningVariable(valueRangeProviderRefs="values", allowsUnassigned=true) public Integer c;
        public Entity() {}
        Entity(Integer[] row) { a=row[0]; b=row[1]; c=row[2]; }
    }
    @PlanningSolution public static class Problem {
        @ValueRangeProvider(id="values") public List<Integer> values=List.of(-2,-1,0,1,2,3);
        @PlanningEntityCollectionProperty public List<Entity> entities=new ArrayList<>();
        @PlanningScore public SimpleScore score;
        public Problem() {}
    }
    static Integer value(String s) { return s.equals("-") ? null : Integer.valueOf(s); }
    static int[] indices(String s) { return s.equals("-") ? new int[0] : Arrays.stream(s.split(",")).mapToInt(Integer::parseInt).toArray(); }
    static Integer[] state(Problem p) {
        var result=new Integer[3*p.entities.size()];
        for(int i=0;i<p.entities.size();i++) { var e=p.entities.get(i); result[3*i]=e.a; result[3*i+1]=e.b; result[3*i+2]=e.c; }
        return result;
    }
    static String encode(Integer[] a) { return Arrays.stream(a).map(v -> v==null ? "-" : v.toString()).collect(Collectors.joining(",")); }
    static long checksum(Integer[] a) { long result=0; for(int i=0;i<a.length;i++) result+=(long)(i+1)*(a[i]==null ? 7 : a[i]); return result; }
    static void check(Integer[] actual,Integer[] expected) { if(!Arrays.equals(actual,expected)) throw new IllegalStateException("State mismatch"); }
    static Move<Problem> move(String kind, List<PlanningVariableMetaModel<Problem,Entity,Integer>> variables,
            List<Entity> left,List<Entity> right,Integer target) {
        return kind.equals("c") ? Moves.massChange(variables.getFirst(),Sample.of(left),target)
            : Moves.pillarSwap(variables,Sample.of(left),Sample.of(right));
    }
    public static void main(String[] args) throws Exception {
        int iterations=Integer.parseInt(args[0]),warmup=Integer.parseInt(args[1]),samples=Integer.parseInt(args[2]);
        if(iterations<1 || iterations>1000000 || warmup<0 || samples<1) throw new IllegalArgumentException("Counts");
        var meta=PlanningSolutionMetaModel.of(Problem.class,Entity.class);
        var all=List.of(meta.genuineEntity(Entity.class).basicVariable("a",Integer.class),
                       meta.genuineEntity(Entity.class).basicVariable("b",Integer.class),
                       meta.genuineEntity(Entity.class).basicVariable("c",Integer.class));
        try(var input=new BufferedReader(new InputStreamReader(System.in))) {
            for(String line;(line=input.readLine())!=null;) {
                String[] f=line.split("\t",-1);
                if(f.length!=7 || !(f[1].equals("c") || f[1].equals("s"))) throw new IllegalArgumentException("Protocol");
                var p=new Problem();
                for(String row:f[6].split(";")) {
                    Integer[] values=Arrays.stream(row.split(",")).map(PillarMoveBenchmark::value).toArray(Integer[]::new);
                    if(values.length!=3) throw new IllegalArgumentException("Row width");
                    p.entities.add(new Entity(values));
                }
                int[] columns=indices(f[2]), li=indices(f[3]), ri=indices(f[4]);
                Integer target=value(f[5]);
                var variables=new ArrayList<PlanningVariableMetaModel<Problem,Entity,Integer>>();
                for(int col:columns) variables.add(all.get(col));
                var left=Arrays.stream(li).mapToObj(p.entities::get).toList();
                var right=Arrays.stream(ri).mapToObj(p.entities::get).toList();
                var initial=state(p); var expected=initial.clone();
                for(int col:columns) {
                    for(int row:li) expected[3*row+col]=f[1].equals("c") ? target : initial[3*ri[0]+col];
                    if(f[1].equals("s")) for(int row:ri) expected[3*row+col]=initial[3*li[0]+col];
                }
                var context=MoveTester.build(meta).using(p);
                for(int sample=-warmup;sample<samples;sample++) {
                    Integer[][] trial={null}; Integer[] restored=null; long sum=0;
                    long start=System.nanoTime();
                    for(int i=0;i<iterations;i++) {
                        var movement=move(f[1],variables,left,right,target);
                        context.executeTemporarily(movement,view -> { trial[0]=state(p); check(trial[0],expected); });
                        restored=state(p); check(restored,initial);
                        sum+=31*checksum(trial[0])+checksum(restored);
                    }
                    long elapsed=System.nanoTime()-start;
                    context.execute(move(f[1],variables,left,right,target));
                    var committed=state(p);check(committed,expected);
                    if(sample>=0) System.out.println(f[0]+"\t"+elapsed+"\t"+sum+"\t"+encode(trial[0])+"\t"+encode(restored)+"\t"+encode(committed));
                    for(int i=0;i<p.entities.size();i++) { var e=p.entities.get(i); e.a=initial[3*i];e.b=initial[3*i+1];e.c=initial[3*i+2]; }
                    // Reset the public test context after restoring outside its managed move API.
                    context=MoveTester.build(meta).using(p);
                }
            }
        }
    }
}
