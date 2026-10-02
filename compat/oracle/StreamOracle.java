// Original development adapter; only pinned Community public APIs are used.
import java.io.*;
import java.util.*;
import ai.timefold.solver.core.api.domain.entity.*;
import ai.timefold.solver.core.api.domain.solution.*;
import ai.timefold.solver.core.api.domain.valuerange.ValueRangeProvider;
import ai.timefold.solver.core.api.domain.variable.PlanningVariable;
import ai.timefold.solver.core.api.score.HardSoftScore;
import ai.timefold.solver.core.api.score.stream.*;
import ai.timefold.solver.core.api.solver.*;
import ai.timefold.solver.core.config.solver.SolverConfig;
import static ai.timefold.solver.core.api.score.stream.Joiners.equal;

public final class StreamOracle {
    @PlanningEntity public static class Job {
        public int id;
        @PlanningVariable(valueRangeProviderRefs="values") public Integer value;
        @PlanningPin public boolean pinned;
        public long[] weights;
        public Job() {}
        Job(int id,Integer value,boolean pinned,long[] weights){this.id=id;this.value=value;this.pinned=pinned;this.weights=weights;}
    }
    public record Edge(int id,int left,int right) {}
    @PlanningSolution public static class Problem {
        @ValueRangeProvider(id="values") @ProblemFactCollectionProperty public List<Integer> values;
        @PlanningEntityCollectionProperty public List<Job> jobs;
        @ProblemFactCollectionProperty public List<Edge> edges;
        @PlanningScore public HardSoftScore score;
        public Problem() {}
    }
    public static class Provider implements ConstraintProvider {
        static long calls;
        @Override public Constraint[] defineConstraints(ConstraintFactory factory) {
            var assigned=factory.forEach(Job.class);
            return new Constraint[]{
                factory.forEach(Edge.class)
                    .join(assigned,equal(Edge::left,j->j.id))
                    .join(assigned,equal((e,a)->e.right(),j->j.id))
                    .filter((e,a,b)->a.value.equals(b.value))
                    .penalize(HardSoftScore.ONE_HARD,(e,a,b)->{calls++;return 1L;}).asConstraint("conflicts"),
                assigned.groupBy(j->j.value,ConstraintCollectors.sum((Job j)->j.weights[j.value]))
                    .filter((v,sum)->sum!=0)
                    .impact(HardSoftScore.ONE_SOFT,(v,sum)->{calls++;return Math.multiplyExact(sum,Math.abs(sum));}).asConstraint("preferences")
            };
        }
    }
    static SolverConfig config() {return new SolverConfig().withSolutionClass(Problem.class)
        .withEntityClasses(Job.class).withConstraintProviderClass(Provider.class).withRandomSeed(20261017L);}
    static Problem parse(String[] fields) {
        if(fields.length!=6)throw new IllegalArgumentException("expected six fields");
        var base=SolverOracle.parse(fields);var p=new Problem();p.values=base.values;p.jobs=new ArrayList<>();p.edges=new ArrayList<>();
        if(base.jobs.size()>10000||base.values.size()>128)throw new IllegalArgumentException("adapter size bound");
        for(var row:base.weights)for(long w:row)if(w < -1000 || w>1000)throw new IllegalArgumentException("adapter weight bound");
        var initial=fields[2].split(",");
        for(int i=0;i<base.jobs.size();i++)p.jobs.add(new Job(i,initial[i].equals("-")?null:Integer.valueOf(initial[i]),base.jobs.get(i).pinned,base.weights[i]));
        for(int i=0;i<base.edges.length;i++)p.edges.add(new Edge(i,base.edges[i][0],base.edges[i][1]));
        return p;
    }
    static Problem copy(Problem p) {
        var result=new Problem();result.values=p.values;result.edges=p.edges;result.jobs=new ArrayList<>();
        for(var j:p.jobs)result.jobs.add(new Job(j.id,j.value,j.pinned,j.weights));return result;
    }
    static HardSoftScore independentScore(Problem p) {
        long hard=0,soft=0;for(var e:p.edges){var a=p.jobs.get(e.left()).value;var b=p.jobs.get(e.right()).value;if(a!=null&&a.equals(b))hard--;}
        var groups=new TreeMap<Integer,Long>();for(var j:p.jobs)if(j.value!=null)groups.merge(j.value,j.weights[j.value],Long::sum);
        for(long sum:groups.values())soft+=Math.multiplyExact(sum,Math.abs(sum));return HardSoftScore.of(hard,soft);
    }
    static void emit(String id,int state,Problem p,SolutionManager<Problem,HardSoftScore> manager) {
        var score=manager.update(p,SolutionUpdatePolicy.UPDATE_SCORE_ONLY);
        if(!independentScore(p).equals(HardSoftScore.of(score.hardScore(),score.softScore())))
            throw new IllegalStateException("incorrect score");
        boolean complete=p.jobs.stream().allMatch(j->j.value!=null);
        // Community update exposes scores; match analysis is Enterprise-only.
        System.out.println(id+"\t"+state+"\t"+(complete?1:0)+"\t"+score.hardScore()+"\t"+score.softScore()+"\t-\t-");
    }
    public static void main(String[] args)throws Exception {
        var manager=SolutionManager.<Problem,HardSoftScore>create(SolverFactory.create(config()));
        try(var input=new BufferedReader(new InputStreamReader(System.in))) {
            for(String line;(line=input.readLine())!=null;) {
                var fields=line.split("\t",-1);if(fields.length!=7)throw new IllegalArgumentException("expected seven fields");
                var p=parse(Arrays.copyOf(fields,6));int state=0;emit(fields[0],state++,p,manager);
                if(fields[6].isEmpty())continue;
                for(var operation:fields[6].split(";")) {
                    var bits=operation.split(":",-1);if(bits.length!=3||!(bits[0].equals("t")||bits[0].equals("c")))throw new IllegalArgumentException("invalid operation");
                    var job=p.jobs.get(Integer.parseInt(bits[1]));var old=job.value;
                    var value=bits[2].equals("-")?null:Integer.valueOf(bits[2]);
                    if(value!=null&&!p.values.contains(value)||job.pinned&&!Objects.equals(old,value))throw new IllegalArgumentException("invalid move");
                    job.value=value;emit(fields[0],state++,p,manager);
                    if(bits[0].equals("t")){job.value=old;emit(fields[0],state++,p,manager);}
                }
            }
        }
    }
}
