// Original bounded mixed warm/cold-start benchmark; pinned Community public APIs only.
import java.io.*;
import java.util.*;
import ai.timefold.solver.core.api.domain.entity.*;
import ai.timefold.solver.core.api.domain.solution.*;
import ai.timefold.solver.core.api.domain.valuerange.ValueRangeProvider;
import ai.timefold.solver.core.api.domain.variable.*;
import ai.timefold.solver.core.api.score.HardSoftScore;
import ai.timefold.solver.core.api.score.calculator.EasyScoreCalculator;
import ai.timefold.solver.core.api.solver.SolverFactory;
import ai.timefold.solver.core.config.constructionheuristic.ConstructionHeuristicPhaseConfig;
import ai.timefold.solver.core.config.constructionheuristic.placer.QueuedEntityPlacerConfig;
import ai.timefold.solver.core.config.constructionheuristic.placer.QueuedValuePlacerConfig;
import ai.timefold.solver.core.config.heuristic.selector.common.SelectionOrder;
import ai.timefold.solver.core.config.heuristic.selector.entity.EntitySelectorConfig;
import ai.timefold.solver.core.config.heuristic.selector.value.ValueSelectorConfig;
import ai.timefold.solver.core.config.heuristic.selector.move.composite.UnionMoveSelectorConfig;
import ai.timefold.solver.core.config.heuristic.selector.move.generic.ChangeMoveSelectorConfig;
import ai.timefold.solver.core.config.heuristic.selector.move.generic.list.*;
import ai.timefold.solver.core.config.localsearch.LocalSearchPhaseConfig;
import ai.timefold.solver.core.config.localsearch.decider.acceptor.*;
import ai.timefold.solver.core.config.localsearch.decider.forager.*;
import ai.timefold.solver.core.config.solver.*;
import ai.timefold.solver.core.config.solver.termination.TerminationConfig;

public final class MixedSolverBenchmark {
    @PlanningEntity public static class Vehicle {
        @PlanningVariable(valueRangeProviderRefs="depots") public Integer depot;
        @PlanningListVariable(valueRangeProviderRefs="customers") public List<Customer> route=new ArrayList<>();
        @PlanningPin public boolean pinned;
        @PlanningPinToIndex public int prefix;
        public int id,capacity;
        public Vehicle() {}
    }
    @PlanningEntity public static class Customer {
        public int id,x,y;
        @InverseRelationShadowVariable(sourceVariableName="route") public Vehicle owner;
        @IndexShadowVariable(sourceVariableName="route") public Integer index;
        public Customer() {}
    }
    @PlanningSolution public static class Problem {
        @ValueRangeProvider(id="depots") @ProblemFactCollectionProperty public List<Integer> depots=new ArrayList<>();
        @ValueRangeProvider(id="customers") @PlanningEntityCollectionProperty public List<Customer> customers=new ArrayList<>();
        @PlanningEntityCollectionProperty public List<Vehicle> vehicles=new ArrayList<>();
        @ProblemFactProperty public List<int[]> locations=new ArrayList<>();
        @PlanningScore public HardSoftScore score;
        public Problem() {}
    }
    public static class Calculator implements EasyScoreCalculator<Problem,HardSoftScore> {
        static long calls;
        public HardSoftScore calculateScore(Problem p) { calls++;return score(p); }
    }
    static HardSoftScore score(Problem p) {
        long hard=0,distance=0;int assigned=0;
        for(var v:p.vehicles) {
            assigned+=v.route.size();hard-=Math.max(0,v.route.size()-v.capacity);
            if(v.depot==null){hard--;continue;}
            var home=p.locations.get(v.depot);int x=home[0],y=home[1];
            for(var c:v.route) {distance+=Math.abs((long)x-c.x)+Math.abs((long)y-c.y);x=c.x;y=c.y;}
            distance+=Math.abs((long)x-home[0])+Math.abs((long)y-home[1]);
        }
        hard-=p.customers.size()-assigned;
        return HardSoftScore.of(hard,-distance);
    }
    static int[] point(String text) {
        var f=text.split(":",-1);if(f.length!=2)throw new IllegalArgumentException("point");
        int x=Integer.parseInt(f[0]),y=Integer.parseInt(f[1]);if(Math.abs((long)x)>100000||Math.abs((long)y)>100000)throw new IllegalArgumentException("coordinate bound");return new int[]{x,y};
    }
    static Problem parse(String[] f) {
        if((f.length!=10&&f.length!=11)||!f[0].matches("[A-Za-z0-9_-]+"))throw new IllegalArgumentException("ten or eleven fields");
        if(f.length==11)qualityFloor(f[10]);
        var p=new Problem();for(var text:f[1].split(",",-1)){var xy=point(text);var c=new Customer();c.id=p.customers.size();c.x=xy[0];c.y=xy[1];p.customers.add(c);}
        for(var text:f[2].split(",",-1)){p.depots.add(p.depots.size());p.locations.add(point(text));}
        var caps=f[3].split(",",-1);var depots=f[4].split(",",-1);var routes=f[5].split(";",-1);var prefixes=f[7].split(",",-1);
        if(caps.length==0||caps.length>8||p.customers.size()>48||p.depots.size()>8||depots.length!=caps.length||routes.length!=caps.length||prefixes.length!=caps.length||f[6].length()!=caps.length)throw new IllegalArgumentException("shape");
        for(int i=0;i<caps.length;i++) {
            var v=new Vehicle();v.id=i;v.capacity=Integer.parseInt(caps[i]);v.depot=Integer.valueOf(depots[i]);v.prefix=Integer.parseInt(prefixes[i]);
            char pin=f[6].charAt(i);if(pin!='0'&&pin!='1')throw new IllegalArgumentException("pin");v.pinned=pin=='1';
            if(v.capacity<0||!p.depots.contains(v.depot))throw new IllegalArgumentException("capacity/depot");
            if(!routes[i].equals("-"))for(var id:routes[i].split(",",-1))v.route.add(p.customers.get(Integer.parseInt(id)));
            if(v.prefix<0||v.prefix>v.route.size()||(v.pinned&&v.prefix!=0))throw new IllegalArgumentException("prefix");p.vehicles.add(v);
        }
        validate(p,p,false);if(score(p).hardScore()!=0)throw new IllegalArgumentException("feasible warm start required");return p;
    }
    static HardSoftScore qualityFloor(String text) {
        var parts=text.split(":",-1);
        if(parts.length!=2||!parts[0].matches("-?[0-9]+")||!parts[1].matches("-?[0-9]+"))throw new IllegalArgumentException("quality floor hard:soft");
        return HardSoftScore.of(Long.parseLong(parts[0]),Long.parseLong(parts[1]));
    }
    static Problem copy(Problem original) {
        var p=new Problem();p.depots.addAll(original.depots);p.locations.addAll(original.locations);
        for(var c:original.customers){var n=new Customer();n.id=c.id;n.x=c.x;n.y=c.y;p.customers.add(n);}
        for(var v:original.vehicles){var n=new Vehicle();n.id=v.id;n.capacity=v.capacity;n.depot=v.depot;n.pinned=v.pinned;n.prefix=v.prefix;for(var c:v.route)n.route.add(p.customers.get(c.id));p.vehicles.add(n);}return p;
    }
    static void prepareCold(Problem p) {
        for(var v:p.vehicles)if(!v.pinned){v.depot=null;v.route.subList(v.prefix,v.route.size()).clear();}
    }
    static void validate(Problem p,Problem original,boolean shadows) {
        var seen=new HashSet<Integer>();
        if(p.vehicles.size()!=original.vehicles.size())throw new IllegalStateException("owner count");
        for(var v:p.vehicles){var before=original.vehicles.get(v.id);if(!p.depots.contains(v.depot))throw new IllegalStateException("depot");
            if(v.pinned&&(!v.depot.equals(before.depot)||!ids(v.route).equals(ids(before.route))))throw new IllegalStateException("pin");
            if(!ids(v.route).subList(0,v.prefix).equals(ids(before.route).subList(0,v.prefix)))throw new IllegalStateException("prefix");
            for(int i=0;i<v.route.size();i++){var c=v.route.get(i);if(!seen.add(c.id)||c.id<0||c.id>=p.customers.size()||(shadows&&(c.owner!=v||!Integer.valueOf(i).equals(c.index))))throw new IllegalStateException("ownership/shadow");}
        }
        if(seen.size()!=p.customers.size())throw new IllegalStateException("incomplete");
        if(shadows&&(!score(p).equals(p.score)||p.score.hardScore()!=0||p.score.compareTo(score(original))<0))throw new IllegalStateException("score/quality");
    }
    static List<Integer> ids(List<Customer> route){return route.stream().map(c->c.id).toList();}
    static SolverFactory<Problem> factory(int steps,long evaluations,boolean cold) {
        var change=new ChangeMoveSelectorConfig().withEntitySelectorConfig(new EntitySelectorConfig().withEntityClass(Vehicle.class))
            .withValueSelectorConfig(new ValueSelectorConfig().withVariableName("depot").withSelectionOrder(SelectionOrder.ORIGINAL));
        change.setSelectionOrder(SelectionOrder.ORIGINAL);
        var moves=new UnionMoveSelectorConfig();moves.setSelectionOrder(SelectionOrder.ORIGINAL);
        moves.setMoveSelectorList(List.of(change,new ListChangeMoveSelectorConfig().withSelectionOrder(SelectionOrder.ORIGINAL),new ListSwapMoveSelectorConfig().withSelectionOrder(SelectionOrder.ORIGINAL)));
        var phase=new LocalSearchPhaseConfig().withMoveSelectorConfig(moves)
            .withAcceptorConfig(new LocalSearchAcceptorConfig().withAcceptorTypeList(List.of(AcceptorType.HILL_CLIMBING)))
            .withForagerConfig(new LocalSearchForagerConfig().withAcceptedCountLimit(Integer.MAX_VALUE).withPickEarlyType(LocalSearchPickEarlyType.NEVER).withBreakTieRandomly(false))
            .withTerminationConfig(new TerminationConfig().withStepCountLimit(steps).withScoreCalculationCountLimit(evaluations));
        var config=new SolverConfig().withSolutionClass(Problem.class).withEntityClasses(Vehicle.class,Customer.class)
            .withEasyScoreCalculatorClass(Calculator.class).withEnvironmentMode(EnvironmentMode.NO_ASSERT).withRandomSeed(20261017L).withPhases(phase);
        if(cold) {
            // Community DefaultSolverTest#solveMixedModelCustomMove uses these two placers.
            // The entity placer initializes basic variables; the value placer initializes route.
            var basics=new ConstructionHeuristicPhaseConfig().withEntityPlacerConfig(new QueuedEntityPlacerConfig()
                .withEntitySelectorConfig(new EntitySelectorConfig().withEntityClass(Vehicle.class).withSelectionOrder(SelectionOrder.ORIGINAL)));
            var lists=new ConstructionHeuristicPhaseConfig().withEntityPlacerConfig(new QueuedValuePlacerConfig()
                .withEntityClass(Vehicle.class).withValueSelectorConfig(new ValueSelectorConfig().withVariableName("route").withSelectionOrder(SelectionOrder.ORIGINAL)));
            config.withPhases(basics,lists,phase)
                .withTerminationConfig(new TerminationConfig().withScoreCalculationCountLimit(evaluations));
        }
        return SolverFactory.create(config);
    }
    public static void main(String[] args)throws Exception {
        if(args.length!=3&&(args.length!=4||!args[3].equals("cold")))throw new IllegalArgumentException("iterations warmup samples [cold]");
        boolean cold=args.length==4;
        int iterations=Integer.parseInt(args[0]),warmup=Integer.parseInt(args[1]),samples=Integer.parseInt(args[2]);
        if(iterations<1||iterations>1000||warmup<0||warmup>100||samples<1||samples>1000)throw new IllegalArgumentException("run bounds");
        try(var input=new BufferedReader(new InputStreamReader(System.in))){var f=input.readLine().split("\t",-1);if(f.length!=(cold?11:10))throw new IllegalArgumentException("input fields must match warm/cold mode");var original=parse(f);var floor=cold?qualityFloor(f[10]):null;int steps=Integer.parseInt(f[8]);long evaluations=Long.parseLong(f[9]);
            if(steps<1||steps>128||evaluations<1||evaluations>2000000)throw new IllegalArgumentException("limits");var factory=factory(steps,evaluations,cold);
            for(int sample=0;sample<warmup+samples;sample++){long elapsed=0,hard=0,soft=0,calls=0;Problem last=null;
                for(int i=0;i<iterations;i++){var problem=cold?null:copy(original);Calculator.calls=0;long started=System.nanoTime();if(cold){problem=copy(original);prepareCold(problem);}var result=factory.buildSolver().solve(problem);elapsed+=System.nanoTime()-started;calls+=Calculator.calls;
                    validate(result,original,true);if(cold&&result.score.compareTo(floor)<0)throw new IllegalStateException("cold quality floor");hard+=result.score.hardScore();soft+=result.score.softScore();last=result;}
                if(sample>=warmup){var depots=last.vehicles.stream().map(v->v.depot).toList();var routes=last.vehicles.stream().map(v->ids(v.route)).toList();
                    System.out.println("{\"id\":\""+f[0]+"\",\"elapsed_ns\":"+elapsed+",\"score\":["+hard+","+soft+"],\"depots\":"+depots+",\"routes\":"+routes+",\"evaluations\":null,\"steps\":null,\"score_calls\":"+calls+",\"termination\":\"public limits or exhausted neighborhood; detailed reason not exposed\"}");}
            }
        }
    }
}
