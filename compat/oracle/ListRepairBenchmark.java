// Original adapter using only pinned Community public APIs. No production implementation copied.
import java.io.*;
import java.util.*;
import java.lang.ref.Reference;
import ai.timefold.solver.core.api.domain.entity.PlanningEntity;
import ai.timefold.solver.core.api.domain.solution.*;
import ai.timefold.solver.core.api.domain.valuerange.ValueRangeProvider;
import ai.timefold.solver.core.api.domain.variable.*;
import ai.timefold.solver.core.api.score.HardSoftScore;
import ai.timefold.solver.core.api.score.calculator.EasyScoreCalculator;
import ai.timefold.solver.core.api.solver.SolverFactory;
import ai.timefold.solver.core.config.heuristic.selector.move.generic.list.ListRuinRecreateMoveSelectorConfig;
import ai.timefold.solver.core.config.localsearch.LocalSearchPhaseConfig;
import ai.timefold.solver.core.config.localsearch.decider.acceptor.LocalSearchAcceptorConfig;
import ai.timefold.solver.core.config.localsearch.decider.acceptor.stepcountinghillclimbing.StepCountingHillClimbingType;
import ai.timefold.solver.core.config.localsearch.decider.forager.LocalSearchForagerConfig;
import ai.timefold.solver.core.config.localsearch.decider.forager.LocalSearchPickEarlyType;
import ai.timefold.solver.core.config.solver.*;
import ai.timefold.solver.core.config.solver.monitoring.SolverMetric;
import ai.timefold.solver.core.config.solver.termination.TerminationConfig;
import io.micrometer.core.instrument.Metrics;
import io.micrometer.core.instrument.simple.SimpleMeterRegistry;

public final class ListRepairBenchmark {
    interface Value { int id(); Owner owner(); Integer index(); }
    interface Owner { List<? extends Value> values(); }
    @PlanningEntity public static class RequiredOwner implements Owner {
        @PlanningListVariable public List<RequiredValue> values = new ArrayList<>();
        public RequiredOwner() {} public List<RequiredValue> values() { return values; }
    }
    @PlanningEntity public static class RequiredValue implements Value {
        public int id;
        @InverseRelationShadowVariable(sourceVariableName="values") public RequiredOwner owner;
        @IndexShadowVariable(sourceVariableName="values") public Integer index;
        public RequiredValue() {} public int id() { return id; }
        public Owner owner(){return owner;} public Integer index(){return index;}
    }
    @PlanningEntity public static class OptionalOwner implements Owner {
        @PlanningListVariable(allowsUnassignedValues=true) public List<OptionalValue> values = new ArrayList<>();
        public OptionalOwner() {} public List<OptionalValue> values() { return values; }
    }
    @PlanningEntity public static class OptionalValue implements Value {
        public int id;
        @InverseRelationShadowVariable(sourceVariableName="values") public OptionalOwner owner;
        @IndexShadowVariable(sourceVariableName="values") public Integer index;
        public OptionalValue() {} public int id() { return id; }
        public Owner owner(){return owner;} public Integer index(){return index;}
    }
    @PlanningSolution public static class RequiredProblem {
        @PlanningEntityCollectionProperty public List<RequiredOwner> owners = new ArrayList<>();
        @PlanningEntityCollectionProperty @ValueRangeProvider public List<RequiredValue> values = new ArrayList<>();
        @ProblemFactProperty public Data data;
        @PlanningScore public HardSoftScore score;
        public RequiredProblem() {}
    }
    @PlanningSolution public static class OptionalProblem {
        @PlanningEntityCollectionProperty public List<OptionalOwner> owners = new ArrayList<>();
        @PlanningEntityCollectionProperty @ValueRangeProvider public List<OptionalValue> values = new ArrayList<>();
        @ProblemFactProperty public Data data;
        @PlanningScore public HardSoftScore score;
        public OptionalProblem() {}
    }
    public static class RequiredCalculator implements EasyScoreCalculator<RequiredProblem,HardSoftScore> {
        public HardSoftScore calculateScore(RequiredProblem p) { calls++; return score(p.data,p.owners); }
    }
    public static class OptionalCalculator implements EasyScoreCalculator<OptionalProblem,HardSoftScore> {
        public HardSoftScore calculateScore(OptionalProblem p) { calls++; return score(p.data,p.owners); }
    }
    static long calls;
    public static final class Data {
        String id; boolean optional; int[] caps, missing; int[][] costs;
        static int[] numbers(String s) { return Arrays.stream(s.split(",",-1)).mapToInt(Integer::parseInt).toArray(); }
        static Data parse(String line) {
            var f=line.split("\t",-1); if(f.length!=5 || f[0].isEmpty() || !Set.of("0","1").contains(f[1])) throw new IllegalArgumentException("shape");
            var d=new Data();d.id=f[0];d.optional=f[1].equals("1");d.caps=numbers(f[2]);d.missing=numbers(f[4]);
            d.costs=Arrays.stream(f[3].split(";",-1)).map(Data::numbers).toArray(int[][]::new);
            if(d.caps.length<2 || d.caps.length>4 || d.costs.length<2 || d.costs.length>96 || d.missing.length!=d.costs.length || d.caps[0]!=0) throw new IllegalArgumentException("sizes");
            for(int c:d.caps) if(c<0 || c>96)throw new IllegalArgumentException("capacity");
            for(int v=0;v<d.costs.length;v++) {
                var row=d.costs[v];if(row.length!=d.caps.length || d.missing[v]<0 || d.missing[v]>row[0])throw new IllegalArgumentException("cost shape");
                for(int c:row)if(c<0 || c>10000 || c>row[0])throw new IllegalArgumentException("initial must be worst");
            }
            return d;
        }
    }
    static HardSoftScore score(Data d,List<? extends Owner> owners) {
        long cost=0;var seen=new boolean[d.costs.length];
        for(int e=0;e<owners.size();e++) {
            var values=owners.get(e).values();long excess=Math.max(0,values.size()-d.caps[e]);cost+=excess*excess;
            int previous=-1;
            for(var value:values) {
                int v=value.id();cost+=d.costs[v][e];seen[v]=true;
                if(previous>=0 && previous<v)cost+=2;previous=v;
            }
        }
        for(int v=0;v<seen.length;v++)if(!seen[v])cost+=d.missing[v];
        return HardSoftScore.of(0,-cost);
    }
    static RequiredProblem required(Data d) {
        var p=new RequiredProblem();p.data=d;
        for(int e=0;e<d.caps.length;e++)p.owners.add(new RequiredOwner());
        for(int i=0;i<d.costs.length;i++){var v=new RequiredValue();v.id=i;p.values.add(v);p.owners.get(0).values.add(v);}
        return p;
    }
    static OptionalProblem optional(Data d) {
        var p=new OptionalProblem();p.data=d;
        for(int e=0;e<d.caps.length;e++)p.owners.add(new OptionalOwner());
        for(int i=0;i<d.costs.length;i++){var v=new OptionalValue();v.id=i;p.values.add(v);p.owners.get(0).values.add(v);}
        return p;
    }
    static void validate(Data d,List<? extends Owner> owners,HardSoftScore claimed) {
        var seen=new HashSet<Integer>();
        for(var owner:owners)for(int index=0;index<owner.values().size();index++) {
            var value=owner.values().get(index);
            int v=value.id();if(v<0 || v>=d.costs.length || !seen.add(v))throw new IllegalStateException("invalid value");
            if(value.owner()!=owner || !Integer.valueOf(index).equals(value.index()))throw new IllegalStateException("invalid metadata");
        }
        if(owners.size()!=d.caps.length || (!d.optional && seen.size()!=d.costs.length))throw new IllegalStateException("incomplete");
        if(!score(d,owners).equals(claimed))throw new IllegalStateException("score mismatch");
        long worst=(long)d.costs.length*d.costs.length+2L*(d.costs.length-1);
        for(var row:d.costs)worst+=row[0];
        if(claimed.softScore() < -worst)throw new IllegalStateException("regressed initial score");
    }
    static String encode(List<? extends Owner> owners) {
        return String.join(";",owners.stream().map(o->o.values().isEmpty()?"-":String.join(",",o.values().stream().map(v->Integer.toString(v.id())).toList())).toList());
    }
    public static void main(String[] args)throws Exception {
        if(args.length!=4 && args.length!=5)throw new IllegalArgumentException("group steps warmup samples [minimum]");
        int group=Integer.parseInt(args[0]),steps=Integer.parseInt(args[1]),warmup=Integer.parseInt(args[2]),samples=Integer.parseInt(args[3]);
        if(group<1 || steps<1 || steps==Integer.MAX_VALUE || warmup<0 || samples<1)throw new IllegalArgumentException("counts");
        int minimum=args.length==5?Integer.parseInt(args[4]):1;
        if(minimum<1 || minimum>group)throw new IllegalArgumentException("minimum");
        Math.addExact(warmup,samples);
        try(var input=new BufferedReader(new InputStreamReader(System.in))) {
            for(String line;(line=input.readLine())!=null;) {
                var d=Data.parse(line);if(group>d.costs.length)throw new IllegalArgumentException("group");
                var phase=new LocalSearchPhaseConfig().withMoveSelectorConfig(new ListRuinRecreateMoveSelectorConfig().withMinimumRuinedCount(minimum).withMaximumRuinedCount(group))
                    .withAcceptorConfig(new LocalSearchAcceptorConfig().withStepCountingHillClimbingSize(steps+1).withStepCountingHillClimbingType(StepCountingHillClimbingType.STEP))
                    .withForagerConfig(new LocalSearchForagerConfig().withAcceptedCountLimit(1).withPickEarlyType(LocalSearchPickEarlyType.NEVER).withBreakTieRandomly(false))
                    .withTerminationConfig(new TerminationConfig().withStepCountLimit(steps));
                var cfg=new SolverConfig().withRandomSeed(20261225L).withEnvironmentMode(EnvironmentMode.NO_ASSERT).withPhases(phase);
                if(d.optional)cfg.withSolutionClass(OptionalProblem.class).withEntityClasses(OptionalOwner.class,OptionalValue.class).withEasyScoreCalculatorClass(OptionalCalculator.class);
                else cfg.withSolutionClass(RequiredProblem.class).withEntityClasses(RequiredOwner.class,RequiredValue.class).withEasyScoreCalculatorClass(RequiredCalculator.class);
                var factory=SolverFactory.create(cfg);
                for(int run=0;run<warmup+samples;run++) {
                    Metrics.globalRegistry.clear();var registry=new SimpleMeterRegistry();
                    try {
                        Metrics.addRegistry(registry);calls=0;
                        long start=System.nanoTime();var solver=factory.buildSolver();
                        List<? extends Owner> owners;HardSoftScore result;
                        if(d.optional){var solved=(OptionalProblem)solver.solve(optional(d));owners=solved.owners;result=solved.score;}
                        else {var solved=(RequiredProblem)solver.solve(required(d));owners=solved.owners;result=solved.score;}
                        long scoreCalls=calls;validate(d,owners,result);long elapsed=System.nanoTime()-start;
                        SolverMetric.MOVE_EVALUATION_COUNT.register(solver);
                        double repairs=registry.get("timefold.solver.move.evaluation.count").gauge().value();Reference.reachabilityFence(solver);
                        if(repairs<0 || repairs>steps || repairs!=Math.rint(repairs))throw new IllegalStateException("unexpected repairs: "+repairs);
                        if(run>=warmup)System.out.println(d.id+"\t"+elapsed+"\t"+result.hardScore()+"\t"+result.softScore()+"\t"+encode(owners)+"\t"+scoreCalls+"\t"+(long)repairs);
                    } finally {Metrics.removeRegistry(registry);Metrics.globalRegistry.clear();registry.close();}
                }
            }
        }
    }
}
