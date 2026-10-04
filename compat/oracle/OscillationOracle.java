// Original diagnostic adapter using public Community solver configuration and
// documented Micrometer STEP_SCORE observations. No internal implementation API.
import java.io.*;
import java.util.*;
import ai.timefold.solver.core.api.domain.entity.PlanningEntity;
import ai.timefold.solver.core.api.domain.solution.*;
import ai.timefold.solver.core.api.domain.valuerange.ValueRangeProvider;
import ai.timefold.solver.core.api.domain.variable.PlanningVariable;
import ai.timefold.solver.core.api.score.HardMediumSoftScore;
import ai.timefold.solver.core.api.score.calculator.EasyScoreCalculator;
import ai.timefold.solver.core.api.solver.SolverFactory;
import ai.timefold.solver.core.config.solver.*;
import ai.timefold.solver.core.config.solver.monitoring.*;
import ai.timefold.solver.core.config.solver.termination.TerminationConfig;
import ai.timefold.solver.core.config.localsearch.*;
import ai.timefold.solver.core.config.localsearch.decider.acceptor.*;
import ai.timefold.solver.core.config.localsearch.decider.forager.*;
import ai.timefold.solver.core.config.heuristic.selector.move.generic.ChangeMoveSelectorConfig;
import ai.timefold.solver.core.config.heuristic.selector.common.SelectionOrder;
import io.micrometer.core.instrument.Metrics;
import io.micrometer.core.instrument.simple.SimpleMeterRegistry;

public final class OscillationOracle {
    @PlanningEntity public static class Job {
        @PlanningVariable(valueRangeProviderRefs="values") public Integer value;
        public Job() {} Job(int value) {this.value=value;}
    }
    @PlanningSolution public static class Problem {
        @ProblemFactCollectionProperty @ValueRangeProvider(id="values") public List<Integer> values;
        @PlanningEntityCollectionProperty public List<Job> jobs;
        @PlanningScore public HardMediumSoftScore score;
        public HardMediumSoftScore[] scores;
        public Problem() {}
    }
    public static class Calculator implements EasyScoreCalculator<Problem,HardMediumSoftScore> {
        public HardMediumSoftScore calculateScore(Problem p) {return p.scores[p.jobs.getFirst().value];}
    }
    static SolverFactory<Problem> factory(String mode,int steps) {
        var ranking=switch(mode) {
            case "highest" -> FinalistPodiumType.HIGHEST_SCORE;
            case "last" -> FinalistPodiumType.STRATEGIC_OSCILLATION_BY_LEVEL;
            case "best" -> FinalistPodiumType.STRATEGIC_OSCILLATION_BY_LEVEL_ON_BEST_SCORE;
            default -> throw new IllegalArgumentException("unknown ranking");
        };
        var phase=new LocalSearchPhaseConfig()
            .withMoveSelectorConfig(new ChangeMoveSelectorConfig().withSelectionOrder(SelectionOrder.ORIGINAL))
            .withAcceptorConfig(new LocalSearchAcceptorConfig().withSimulatedAnnealingStartingTemperature(
                "9223372036854775807hard/9223372036854775807medium/9223372036854775807soft"))
            .withForagerConfig(new LocalSearchForagerConfig().withAcceptedCountLimit(Integer.MAX_VALUE)
                .withPickEarlyType(LocalSearchPickEarlyType.NEVER).withBreakTieRandomly(false).withFinalistPodiumType(ranking))
            .withTerminationConfig(new TerminationConfig().withStepCountLimit(steps));
        return SolverFactory.create(new SolverConfig().withSolutionClass(Problem.class).withEntityClasses(Job.class)
            .withEasyScoreCalculatorClass(Calculator.class).withEnvironmentMode(EnvironmentMode.NO_ASSERT)
            .withRandomSeed(20261219L).withPhases(phase)
            .withMonitoringConfig(new MonitoringConfig().withSolverMetricList(List.of(SolverMetric.STEP_SCORE, SolverMetric.MOVE_COUNT_PER_STEP))));
    }
    static long stepLevel(SimpleMeterRegistry registry,String level) {
        double value=registry.get("timefold.solver.step.score."+level+".score").gauge().value();
        if (!Double.isFinite(value) || value!=Math.rint(value) || Math.abs(value)>8)
            throw new IllegalStateException("missing or inexact step score");
        return (long)value;
    }
    public static void main(String[] args) throws Exception {
        var factories=new HashMap<String,SolverFactory<Problem>>();
        try(var input=new BufferedReader(new InputStreamReader(System.in))) {
            for(String line;(line=input.readLine())!=null;) {
                var f=line.split("\\t",-1);
                if(f.length!=4) throw new IllegalArgumentException("expected id, ranking, steps, score triples");
                int steps=Integer.parseInt(f[2]);
                if(steps<1 || steps>4) throw new IllegalArgumentException("bounded steps required");
                var triples=f[3].split(";");
                if(triples.length<2 || triples.length>16) throw new IllegalArgumentException("bounded value count required");
                var p=new Problem();p.values=new ArrayList<>();p.jobs=new ArrayList<>(List.of(new Job(0)));
                p.scores=new HardMediumSoftScore[triples.length];
                for(int i=0;i<triples.length;i++) {
                    var levels=triples[i].split(",");
                    if(levels.length!=3) throw new IllegalArgumentException("three score levels required");
                    long[] scores=new long[3];
                    for(int j=0;j<3;j++) {
                        scores[j]=Long.parseLong(levels[j]);
                        if(Math.abs(scores[j])>8 || scores[j]==Long.MIN_VALUE) throw new IllegalArgumentException("bounded score required");
                    }
                    p.values.add(i);p.scores[i]=HardMediumSoftScore.of(scores[0],scores[1],scores[2]);
                }
                var factory=factories.computeIfAbsent(f[1]+":"+steps,k->factory(f[1],steps));
                // This standalone process owns its global registry. Clear old gauges
                // so each observation belongs only to this solve, never a previous case.
                Metrics.globalRegistry.clear();
                var registry=new SimpleMeterRegistry();
                try {
                    Metrics.addRegistry(registry);
                    try {
                        var best=factory.buildSolver().solve(p);
                        int value=best.jobs.getFirst().value;
                        double selected=registry.get("timefold.solver.step.move.count.selected").gauge().value();
                        double accepted=registry.get("timefold.solver.step.move.count.accepted").gauge().value();
                        if(selected!=triples.length-1 || accepted!=selected)
                            throw new IllegalStateException("incomplete or rejected candidate set");
                        if(!best.score.equals(p.scores[value])) throw new IllegalStateException("invalid retained best");
                        System.out.println(f[0]+"\t"+stepLevel(registry,"hard")+"\t"+stepLevel(registry,"medium")+"\t"+stepLevel(registry,"soft")
                            +"\t"+best.score.hardScore()+"\t"+best.score.mediumScore()+"\t"+best.score.softScore()+"\t"+value+"\t"+(int)selected+"\t"+(int)accepted);
                    } finally {Metrics.removeRegistry(registry);Metrics.globalRegistry.clear();}
                } finally {registry.close();}
            }
        }
    }
}
