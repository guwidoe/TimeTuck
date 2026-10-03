// Original development adapter; pinned Community public APIs only.
import java.io.*;
import java.util.*;
import java.util.stream.Collectors;
import ai.timefold.solver.core.api.domain.entity.PlanningEntity;
import ai.timefold.solver.core.api.domain.solution.*;
import ai.timefold.solver.core.api.domain.valuerange.ValueRangeProvider;
import ai.timefold.solver.core.api.domain.variable.*;
import ai.timefold.solver.core.api.score.HardSoftScore;
import ai.timefold.solver.core.api.score.calculator.EasyScoreCalculator;
import ai.timefold.solver.core.api.solver.SolverFactory;
import ai.timefold.solver.core.config.exhaustivesearch.*;
import ai.timefold.solver.core.config.heuristic.selector.entity.EntitySorterManner;
import ai.timefold.solver.core.config.heuristic.selector.value.ValueSorterManner;
import ai.timefold.solver.core.config.solver.*;

public final class MixedSolverOracle {
    @PlanningEntity public static class Vehicle {
        @PlanningVariable(valueRangeProviderRefs="depots") public Integer depot;
        @PlanningListVariable(valueRangeProviderRefs="customers") public List<Customer> route=new ArrayList<>();
        public int capacity;
        public Vehicle() {}
    }
    @PlanningEntity public static class Customer {
        public int id, point;
        @InverseRelationShadowVariable(sourceVariableName="route") public Vehicle owner;
        @IndexShadowVariable(sourceVariableName="route") public Integer index;
        public Customer() {}
    }
    @PlanningSolution public static class Problem {
        @ValueRangeProvider(id="depots") @ProblemFactCollectionProperty public List<Integer> depots=new ArrayList<>();
        @ValueRangeProvider(id="customers") @PlanningEntityCollectionProperty public List<Customer> customers=new ArrayList<>();
        @PlanningEntityCollectionProperty public List<Vehicle> vehicles=new ArrayList<>();
        @PlanningScore public HardSoftScore score;
        public Problem() {}
    }
    public static class Calculator implements EasyScoreCalculator<Problem,HardSoftScore> {
        public HardSoftScore calculateScore(Problem p) {
            long hard=0, distance=0;
            for(var v:p.vehicles) {
                hard-=Math.max(0,v.route.size()-v.capacity);
                int depot=v.depot==null?0:v.depot, previous=depot;
                for(var c:v.route) {distance+=Math.abs((long)c.point-previous);previous=c.point;}
                distance+=Math.abs((long)previous-depot);
            }
            return HardSoftScore.of(hard,-distance);
        }
    }
    public static void main(String[] args)throws Exception {
        var config=new SolverConfig().withSolutionClass(Problem.class).withEntityClasses(Vehicle.class,Customer.class)
            .withEasyScoreCalculatorClass(Calculator.class).withEnvironmentMode(EnvironmentMode.NO_ASSERT)
            .withRandomSeed(20261123L).withPhases(new ExhaustiveSearchPhaseConfig()
                .withExhaustiveSearchType(ExhaustiveSearchType.BRUTE_FORCE)
                .withEntitySorterManner(EntitySorterManner.NONE).withValueSorterManner(ValueSorterManner.NONE));
        var factory=SolverFactory.<Problem>create(config);
        try(var input=new BufferedReader(new InputStreamReader(System.in))) {
            for(String line;(line=input.readLine())!=null;) {
                var f=line.split("\t",-1);if(f.length!=4)throw new IllegalArgumentException("four fields required");
                var p=new Problem();
                for(var v:f[1].split(","))p.depots.add(Integer.valueOf(v));
                for(var v:f[2].split(",")){var c=new Customer();c.id=p.customers.size();c.point=Integer.parseInt(v);p.customers.add(c);}
                for(var v:f[3].split(",")){var e=new Vehicle();e.capacity=Integer.parseInt(v);p.vehicles.add(e);}
                var solved=factory.buildSolver().solve(p);
                if(!new Calculator().calculateScore(solved).equals(solved.score))throw new IllegalStateException("score mismatch");
                var seen=new HashSet<Integer>();
                for(var v:solved.vehicles) {
                    if(v.depot==null || !solved.depots.contains(v.depot))throw new IllegalStateException("depot");
                    for(int i=0;i<v.route.size();i++) {
                        var c=v.route.get(i);
                        if(!seen.add(c.id)||c.owner!=v||!Integer.valueOf(i).equals(c.index))throw new IllegalStateException("ownership/shadow");
                    }
                }
                if(seen.size()!=solved.customers.size())throw new IllegalStateException("incomplete");
                var depots=solved.vehicles.stream().map(v->v.depot.toString()).collect(Collectors.joining(","));
                var routes=solved.vehicles.stream().map(v->v.route.isEmpty()?"-":v.route.stream().map(c->Integer.toString(c.id)).collect(Collectors.joining(","))).collect(Collectors.joining(";"));
                System.out.println(f[0]+"\t"+solved.score.hardScore()+"\t"+solved.score.softScore()+"\t"+depots+"\t"+routes);
            }
        }
    }
}
