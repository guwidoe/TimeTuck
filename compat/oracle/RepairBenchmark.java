// Original benchmark adapter using only Timefold Community public configuration APIs.
import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.util.ArrayList;
import ai.timefold.solver.core.api.score.HardSoftScore;
import ai.timefold.solver.core.api.solver.SolverFactory;
import java.lang.ref.Reference;
import io.micrometer.core.instrument.Metrics;
import io.micrometer.core.instrument.simple.SimpleMeterRegistry;
import ai.timefold.solver.core.config.heuristic.selector.move.generic.RuinRecreateMoveSelectorConfig;
import ai.timefold.solver.core.config.localsearch.LocalSearchPhaseConfig;
import ai.timefold.solver.core.config.localsearch.decider.acceptor.LocalSearchAcceptorConfig;
import ai.timefold.solver.core.config.localsearch.decider.acceptor.stepcountinghillclimbing.StepCountingHillClimbingType;
import ai.timefold.solver.core.config.localsearch.decider.forager.LocalSearchForagerConfig;
import ai.timefold.solver.core.config.localsearch.decider.forager.LocalSearchPickEarlyType;
import ai.timefold.solver.core.config.solver.EnvironmentMode;
import ai.timefold.solver.core.config.solver.SolverConfig;
import ai.timefold.solver.core.config.solver.monitoring.SolverMetric;
import ai.timefold.solver.core.config.solver.termination.TerminationConfig;

public final class RepairBenchmark {
    public static class CountingCalculator extends SolverOracle.Calculator {
        static long calls;
        @Override public HardSoftScore calculateScore(SolverOracle.Problem problem) {
            calls++;
            return super.calculateScore(problem);
        }
    }
    static SolverOracle.Problem copy(SolverOracle.Problem original) {
        var copy = new SolverOracle.Problem();
        copy.values = original.values;
        copy.edges = original.edges;
        copy.weights = original.weights;
        copy.jobs = new ArrayList<>();
        for (var job : original.jobs) copy.jobs.add(new SolverOracle.Job(job.value, job.pinned));
        return copy;
    }
    public static void main(String[] args) throws Exception {
        if (args.length != 4) throw new IllegalArgumentException("expected group steps warmup samples");
        int group = Integer.parseInt(args[0]), steps = Integer.parseInt(args[1]);
        int warmup = Integer.parseInt(args[2]), samples = Integer.parseInt(args[3]);
        if (group < 1 || steps < 1 || steps == Integer.MAX_VALUE || warmup < 0 || samples < 1)
            throw new IllegalArgumentException("invalid counts");
        Math.addExact(warmup, samples);
        var phase = new LocalSearchPhaseConfig()
            .withMoveSelectorConfig(new RuinRecreateMoveSelectorConfig()
                .withMinimumRuinedCount(group).withMaximumRuinedCount(group))
            .withAcceptorConfig(new LocalSearchAcceptorConfig().withStepCountingHillClimbingSize(steps + 1)
                .withStepCountingHillClimbingType(StepCountingHillClimbingType.STEP))
            .withForagerConfig(new LocalSearchForagerConfig().withAcceptedCountLimit(1)
                .withPickEarlyType(LocalSearchPickEarlyType.NEVER).withBreakTieRandomly(false))
            .withTerminationConfig(new TerminationConfig().withStepCountLimit(steps));
        var solverConfig = new SolverConfig()
            .withSolutionClass(SolverOracle.Problem.class).withEntityClasses(SolverOracle.Job.class)
            .withEasyScoreCalculatorClass(CountingCalculator.class).withRandomSeed(20261222L)
            .withEnvironmentMode(EnvironmentMode.NO_ASSERT).withPhases(phase);
        var factory = SolverFactory.<SolverOracle.Problem>create(solverConfig);
        try (var input = new BufferedReader(new InputStreamReader(System.in))) {
            for (String line; (line = input.readLine()) != null;) {
                String[] fields = line.split("\t", -1);
                if (fields.length != 6) throw new IllegalArgumentException("homogeneous required variables only");
                var original = SolverOracle.parse(fields);
                if (group > original.jobs.size()) throw new IllegalArgumentException("group too large");
                for (var row : original.weights) for (var weight : row)
                    if (weight < row[0]) throw new IllegalArgumentException("initial weight must be worst");
                String[] values = fields[2].split(",");
                for (int i = 0; i < values.length; i++) {
                    int value = Integer.parseInt(values[i]);
                    if (value != 0 || original.jobs.get(i).pinned) throw new IllegalArgumentException("invalid initial value");
                    original.jobs.get(i).value = value;
                }
                for (int run = 0; run < warmup + samples; run++) {
                    Metrics.globalRegistry.clear();
                    var registry = new SimpleMeterRegistry();
                    try {
                        Metrics.addRegistry(registry);
                        CountingCalculator.calls = 0;
                        long started = System.nanoTime();
                        var solver = factory.buildSolver();
                        var solved = solver.solve(copy(original));
                        long calls = CountingCalculator.calls;
                        if (!new SolverOracle.Calculator().calculateScore(solved).equals(solved.score))
                            throw new IllegalStateException("incorrect score");
                        for (int i = 0; i < solved.jobs.size(); i++) {
                            var value = solved.jobs.get(i).value;
                            if (value == null || value < 0 || value >= original.values.size()
                                || (original.jobs.get(i).pinned && !value.equals(original.jobs.get(i).value)))
                                throw new IllegalStateException("invalid result");
                        }
                        if (solved.score.compareTo(new SolverOracle.Calculator().calculateScore(original)) < 0)
                            throw new IllegalStateException("regressed initial score");
                        long elapsed = System.nanoTime() - started;
                        // The solve unregisters this gauge; observe the public retained count afterward.
                        SolverMetric.MOVE_EVALUATION_COUNT.register(solver);
                        double repairs = registry.get("timefold.solver.move.evaluation.count").gauge().value();
                        Reference.reachabilityFence(solver);
                        if (repairs != steps) throw new IllegalStateException("unexpected repair evaluations: " + repairs);
                        if (run >= warmup) {
                            String assignment = String.join(",", solved.jobs.stream().map(j -> j.value.toString()).toList());
                            System.out.println(fields[0] + "\t" + elapsed + "\t" + solved.score.hardScore()
                                + "\t" + solved.score.softScore() + "\t" + assignment + "\t" + calls + "\t" + (long) repairs);
                        }
                    } finally {
                        Metrics.removeRegistry(registry);
                        Metrics.globalRegistry.clear();
                        registry.close();
                    }
                }
            }
        }
    }
}
