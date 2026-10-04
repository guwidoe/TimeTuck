// Original development adapter. Only pinned Community public annotations/configuration.
import java.io.*;
import java.util.*;
import java.util.stream.Collectors;
import ai.timefold.solver.core.api.domain.entity.*;
import ai.timefold.solver.core.api.domain.solution.*;
import ai.timefold.solver.core.api.domain.valuerange.ValueRangeProvider;
import ai.timefold.solver.core.api.domain.variable.*;
import ai.timefold.solver.core.api.score.HardSoftScore;
import ai.timefold.solver.core.api.score.calculator.EasyScoreCalculator;
import ai.timefold.solver.core.api.solver.SolverFactory;
import ai.timefold.solver.core.config.constructionheuristic.*;
import ai.timefold.solver.core.config.constructionheuristic.decider.forager.*;
import ai.timefold.solver.core.config.heuristic.selector.common.SelectionOrder;
import ai.timefold.solver.core.config.heuristic.selector.entity.EntitySorterManner;
import ai.timefold.solver.core.config.heuristic.selector.value.ValueSorterManner;
import ai.timefold.solver.core.config.heuristic.selector.move.generic.list.ListChangeMoveSelectorConfig;
import ai.timefold.solver.core.config.localsearch.LocalSearchPhaseConfig;
import ai.timefold.solver.core.config.localsearch.decider.acceptor.*;
import ai.timefold.solver.core.config.localsearch.decider.forager.*;
import ai.timefold.solver.core.config.solver.*;
import ai.timefold.solver.core.config.solver.termination.TerminationConfig;
import ai.timefold.solver.core.preview.api.neighborhood.*;
import ai.timefold.solver.core.preview.api.move.builtin.TwoOptListMoveProvider;

public final class ListSolverBenchmark {
    @PlanningEntity public static class Vehicle {
        @PlanningListVariable(valueRangeProviderRefs="eligible") public List<Stop> stops = new ArrayList<>();
        @ValueRangeProvider(id="eligible") public List<Stop> eligible = new ArrayList<>();
        @PlanningPin public boolean pinned;
        @PlanningPinToIndex public int prefix;
        public int id, x, y, capacity;
        public Vehicle() {}
    }
    @PlanningEntity public static class Stop {
        public int id, x, y;
        @InverseRelationShadowVariable(sourceVariableName="stops") public Vehicle owner;
        @IndexShadowVariable(sourceVariableName="stops") public Integer index;
        @PreviousElementShadowVariable(sourceVariableName="stops") public Stop previous;
        @NextElementShadowVariable(sourceVariableName="stops") public Stop next;
        public Stop() {}
    }
    @PlanningSolution public static class Problem {
        @PlanningEntityCollectionProperty public List<Stop> stops;
        @PlanningEntityCollectionProperty public List<Vehicle> vehicles;
        @PlanningScore public HardSoftScore score;
        public Problem() {}
    }
    public static final class TwoOptNeighborhood implements NeighborhoodProvider<Problem> {
        public Neighborhood defineNeighborhood(NeighborhoodBuilder<Problem> builder) {
            var variable=builder.getSolutionMetaModel().genuineEntity(Vehicle.class).listVariable("stops",Stop.class);
            return builder.add(new TwoOptListMoveProvider<>(variable,true)).build();
        }
    }
    public static class Calculator implements EasyScoreCalculator<Problem,HardSoftScore> {
        static long calls;
        public HardSoftScore calculateScore(Problem p) { calls++; return score(p); }
    }
    static HardSoftScore score(Problem p) {
        long hard=0, distance=0;
        for(var e:p.vehicles) {
            hard-=Math.max(0,e.stops.size()-e.capacity); int x=e.x,y=e.y;
            for(var v:e.stops) {distance+=Math.abs((long)x-v.x)+Math.abs((long)y-v.y);x=v.x;y=v.y;}
            distance+=Math.abs((long)x-e.x)+Math.abs((long)y-e.y);
        }
        return HardSoftScore.of(hard,-distance);
    }
    static int[] point(String s) {
        var f=s.split(":",-1); if(f.length!=2) throw new IllegalArgumentException("point");
        var p=new int[]{Integer.parseInt(f[0]),Integer.parseInt(f[1])};
        for(int v:p) if(Math.abs((long)v)>1_000_000) throw new IllegalArgumentException("coordinate bound"); return p;
    }
    static Problem parse(String[] f) {
        if((f.length!=7 && f.length!=8) || f[0].isEmpty()) throw new IllegalArgumentException("seven or eight fields required");
        var p=new Problem(); p.stops=new ArrayList<>(); p.vehicles=new ArrayList<>();
        if(!f[1].equals("-")) for(var text:f[1].split(",",-1)) {
            var xy=point(text);var v=new Stop();v.id=p.stops.size();v.x=xy[0];v.y=xy[1];p.stops.add(v);
        }
        var depots=f[2].split(",",-1);var caps=f[3].split(",",-1);var lists=f[4].split(";",-1);var prefixes=f[6].split(",",-1);
        if(depots.length!=caps.length || depots.length!=lists.length || depots.length!=prefixes.length || depots.length!=f[5].length()
            || depots.length>100 || p.stops.size()>1000) throw new IllegalArgumentException("shape");
        var seen=new HashSet<Integer>();
        for(int i=0;i<depots.length;i++) {
            var xy=point(depots[i]);var e=new Vehicle();e.id=i;e.x=xy[0];e.y=xy[1];e.capacity=Integer.parseInt(caps[i]);e.prefix=Integer.parseInt(prefixes[i]);
            char pin=f[5].charAt(i); if(pin!='0' && pin!='1') throw new IllegalArgumentException("pin"); e.pinned=pin=='1';
            if(!lists[i].equals("-")) for(String s:lists[i].split(",",-1)) {
                int id=Integer.parseInt(s);if(id<0 || id>=p.stops.size() || !seen.add(id)) throw new IllegalArgumentException("value");e.stops.add(p.stops.get(id));
            }
            if(e.capacity<0 || e.prefix<0 || e.prefix>e.stops.size() || (e.pinned && e.prefix!=0)) throw new IllegalArgumentException("metadata");
            p.vehicles.add(e);
        }
        String[] ranges=f.length==8?f[7].split(";",-1):null;
        if(ranges!=null && ranges.length!=p.vehicles.size())throw new IllegalArgumentException("range count");
        for(int i=0;i<p.vehicles.size();i++) {
            var e=p.vehicles.get(i);
            if(ranges==null || ranges[i].equals("*")) e.eligible.addAll(p.stops);
            else if(!ranges[i].equals("-")) {
                var distinct=new HashSet<Integer>();
                for(var text:ranges[i].split(",",-1)) {
                    int id=Integer.parseInt(text);
                    if(id<0 || id>=p.stops.size() || !distinct.add(id))throw new IllegalArgumentException("invalid range value");
                    e.eligible.add(p.stops.get(id));
                }
            }
            if(!e.eligible.containsAll(e.stops))throw new IllegalArgumentException("initial value outside entity range");
        }
        return p;
    }
    static Problem copy(Problem p) {
        var c=new Problem();c.stops=new ArrayList<>();c.vehicles=new ArrayList<>();
        for(var v:p.stops) {var w=new Stop();w.id=v.id;w.x=v.x;w.y=v.y;c.stops.add(w);}
        for(var e:p.vehicles) {var d=new Vehicle();d.id=e.id;d.x=e.x;d.y=e.y;d.capacity=e.capacity;d.pinned=e.pinned;d.prefix=e.prefix;
            for(var v:e.stops)d.stops.add(c.stops.get(v.id));for(var v:e.eligible)d.eligible.add(c.stops.get(v.id));c.vehicles.add(d);}
        return c;
    }
    static void validate(Problem p,Problem original) { validate(p, original, score(p)); }
    static void validate(Problem p,Problem original,HardSoftScore independentlyCalculated) {
        if(!independentlyCalculated.equals(p.score)) throw new IllegalStateException("score mismatch");
        var seen=new HashSet<Integer>();
        for(int e=0;e<p.vehicles.size();e++) {
            var vehicle=p.vehicles.get(e);var before=original.vehicles.get(e);
            if(before.pinned && vehicle.stops.size()!=before.stops.size())throw new IllegalStateException("pin length");
            for(int i=0;i<vehicle.stops.size();i++) {
                var v=vehicle.stops.get(i);if(before.eligible.stream().noneMatch(w->w.id==v.id))throw new IllegalStateException("range violation");if(v.id<0 || v.id>=p.stops.size() || !seen.add(v.id))throw new IllegalStateException("ownership");
                if((before.pinned || i<before.prefix) && v.id!=before.stops.get(i).id)throw new IllegalStateException("pin");
                if(v.owner!=vehicle || !Integer.valueOf(i).equals(v.index)
                    || v.previous!=(i==0?null:vehicle.stops.get(i-1)) || v.next!=(i+1==vehicle.stops.size()?null:vehicle.stops.get(i+1)))throw new IllegalStateException("shadow");
            }
            if(vehicle.stops.size()<before.prefix) throw new IllegalStateException("prefix length");
        }
        if(seen.size()!=p.stops.size()) throw new IllegalStateException("incomplete");
    }
    static String lists(Problem p) {return p.vehicles.stream().map(e->e.stops.isEmpty()?"-":e.stops.stream().map(v->""+v.id).collect(Collectors.joining(","))).collect(Collectors.joining(";"));}
    public static void main(String[] args)throws Exception {
        if(args.length!=4)throw new IllegalArgumentException("construct|hill|two-opt steps warmup samples");
        String mode=args[0]; int steps=Integer.parseInt(args[1]),warmup=Integer.parseInt(args[2]),samples=Integer.parseInt(args[3]);
        if(steps<1 || warmup<0 || samples<1)throw new IllegalArgumentException("counts");
        var config=new SolverConfig().withSolutionClass(Problem.class).withEntityClasses(Vehicle.class,Stop.class)
            .withEasyScoreCalculatorClass(Calculator.class).withRandomSeed(20261006L).withEnvironmentMode(EnvironmentMode.NO_ASSERT);
        if(mode.equals("construct")) config.withPhases(new ConstructionHeuristicPhaseConfig().withConstructionHeuristicType(ConstructionHeuristicType.FIRST_FIT)
            .withEntitySorterManner(EntitySorterManner.NONE).withValueSorterManner(ValueSorterManner.NONE)
            .withForagerConfig(new ConstructionHeuristicForagerConfig().withPickEarlyType(ConstructionHeuristicPickEarlyType.NEVER)));
        else if(mode.equals("hill")) config.withPhases(new LocalSearchPhaseConfig()
            .withMoveSelectorConfig(new ListChangeMoveSelectorConfig().withSelectionOrder(SelectionOrder.ORIGINAL))
            .withAcceptorConfig(new LocalSearchAcceptorConfig().withAcceptorTypeList(List.of(AcceptorType.HILL_CLIMBING)))
            .withForagerConfig(new LocalSearchForagerConfig().withAcceptedCountLimit(Integer.MAX_VALUE).withPickEarlyType(LocalSearchPickEarlyType.NEVER).withBreakTieRandomly(false))
            .withTerminationConfig(new TerminationConfig().withStepCountLimit(steps)));
        else if(mode.equals("two-opt")) {
            config.withPreviewFeature(PreviewFeature.NEIGHBORHOODS).withPhases(new LocalSearchPhaseConfig()
                .withMoveProviderClass(TwoOptNeighborhood.class)
                .withAcceptorConfig(new LocalSearchAcceptorConfig().withAcceptorTypeList(List.of(AcceptorType.HILL_CLIMBING)))
                .withForagerConfig(new LocalSearchForagerConfig().withAcceptedCountLimit(1).withPickEarlyType(LocalSearchPickEarlyType.NEVER).withBreakTieRandomly(false))
                .withTerminationConfig(new TerminationConfig().withStepCountLimit(steps).withScoreCalculationCountLimit(10_000L)));
        }
        else throw new IllegalArgumentException("mode");
        var factory=SolverFactory.<Problem>create(config);
        try(var input=new BufferedReader(new InputStreamReader(System.in))) {
            for(String line;(line=input.readLine())!=null;) {
                var f=line.split("\t",-1);var original=parse(f);
                if(!mode.equals("construct") && original.vehicles.stream().mapToInt(e->e.stops.size()).sum()!=original.stops.size()) throw new IllegalArgumentException("complete search input required");
                for(int run=0;run<Math.addExact(warmup,samples);run++) {
                    Calculator.calls=0;long start=System.nanoTime();
                    var solved=factory.buildSolver().solve(copy(original)); long calls=Calculator.calls;
                    validate(solved,original);long elapsed=System.nanoTime()-start;
                    if(run>=warmup)System.out.println(f[0]+"\t"+elapsed+"\t"+solved.score.hardScore()+"\t"+solved.score.softScore()+"\t"+lists(solved)+"\t"+calls);
                }
            }
        }
    }
}
