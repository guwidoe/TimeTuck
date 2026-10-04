// Original development adapter. Only pinned Community public configuration APIs.
import java.io.*;
import java.util.*;
import ai.timefold.solver.core.api.score.HardSoftScore;
import ai.timefold.solver.core.api.score.calculator.EasyScoreCalculator;
import ai.timefold.solver.core.api.solver.SolverFactory;
import ai.timefold.solver.core.config.heuristic.selector.common.SelectionOrder;
import ai.timefold.solver.core.config.heuristic.selector.move.generic.list.kopt.KOptListMoveSelectorConfig;
import ai.timefold.solver.core.config.localsearch.LocalSearchPhaseConfig;
import ai.timefold.solver.core.config.localsearch.decider.acceptor.*;
import ai.timefold.solver.core.config.localsearch.decider.forager.*;
import ai.timefold.solver.core.config.solver.*;
import ai.timefold.solver.core.config.solver.termination.TerminationConfig;

public final class KOptBenchmark {
    public static class Calculator implements EasyScoreCalculator<ListSolverBenchmark.Problem,HardSoftScore> {
        static long calls;
        public HardSoftScore calculateScore(ListSolverBenchmark.Problem p) { calls++; return score(p); }
    }
    static HardSoftScore score(ListSolverBenchmark.Problem p) {
        long distance=0;
        for(var e:p.vehicles) {
            for(int i=0;i<e.stops.size();i++) {
                var a=e.stops.get(i);var b=e.stops.get((i+1)%e.stops.size());
                distance+=Math.abs((long)a.x-b.x)+Math.abs((long)a.y-b.y);
            }
        }
        return HardSoftScore.of(0,-distance);
    }
    public static void main(String[] args)throws Exception {
        if(args.length!=7)throw new IllegalArgumentException("minimum maximum steps evaluations seed warmup samples");
        int minimum=Integer.parseInt(args[0]),k=Integer.parseInt(args[1]),steps=Integer.parseInt(args[2]),warmup=Integer.parseInt(args[5]),samples=Integer.parseInt(args[6]);
        long evaluations=Long.parseLong(args[3]),seed=Long.parseLong(args[4]);
        if(minimum<2 || minimum>k || k>4 || steps<1 || evaluations<1 || seed<0 || warmup<0 || samples<1)throw new IllegalArgumentException("counts");
        var config=new SolverConfig().withSolutionClass(ListSolverBenchmark.Problem.class)
            .withEntityClasses(ListSolverBenchmark.Vehicle.class,ListSolverBenchmark.Stop.class)
            .withEasyScoreCalculatorClass(Calculator.class).withRandomSeed(seed).withEnvironmentMode(EnvironmentMode.NO_ASSERT)
            .withPhases(new LocalSearchPhaseConfig()
                .withMoveSelectorConfig(new KOptListMoveSelectorConfig().withMinimumK(minimum).withMaximumK(k).withSelectionOrder(SelectionOrder.RANDOM))
                .withAcceptorConfig(new LocalSearchAcceptorConfig().withAcceptorTypeList(List.of(AcceptorType.HILL_CLIMBING)))
                .withForagerConfig(new LocalSearchForagerConfig().withAcceptedCountLimit(1).withPickEarlyType(LocalSearchPickEarlyType.NEVER).withBreakTieRandomly(false))
                .withTerminationConfig(new TerminationConfig().withStepCountLimit(steps).withScoreCalculationCountLimit(evaluations)));
        var factory=SolverFactory.<ListSolverBenchmark.Problem>create(config);
        try(var input=new BufferedReader(new InputStreamReader(System.in))) {
            for(String line;(line=input.readLine())!=null;) {
                var fields=line.split("\t",-1);var original=ListSolverBenchmark.parse(fields);
                if(original.vehicles.size()!=1)throw new IllegalArgumentException("one entity required");
                var e=original.vehicles.get(0);
                if(e.pinned || e.prefix!=0 || original.stops.size()<2*k || e.stops.size()!=original.stops.size()
                    || e.capacity<original.stops.size())throw new IllegalArgumentException("complete unpinned sufficiently long route required");
                var initialScore=score(original);
                for(int run=0;run<Math.addExact(warmup,samples);run++) {
                    Calculator.calls=0;long start=System.nanoTime();
                    var solved=factory.buildSolver().solve(ListSolverBenchmark.copy(original));long calls=Calculator.calls;
                    ListSolverBenchmark.validate(solved,original,score(solved));
                    if(solved.score.compareTo(initialScore)<0)throw new IllegalStateException("best regressed");
                    long elapsed=System.nanoTime()-start;
                    if(run>=warmup)System.out.println(fields[0]+"\t"+elapsed+"\t"+solved.score.hardScore()+"\t"+solved.score.softScore()+"\t"+ListSolverBenchmark.lists(solved)+"\t"+calls);
                }
            }
        }
    }
}
