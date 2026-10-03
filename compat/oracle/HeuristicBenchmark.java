// Original benchmark adapter using only Timefold Community public configuration APIs.
import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.util.ArrayList;
import java.util.List;
import ai.timefold.solver.core.api.score.HardSoftScore;
import ai.timefold.solver.core.api.solver.SolverFactory;
import ai.timefold.solver.core.config.heuristic.selector.common.SelectionOrder;
import ai.timefold.solver.core.config.heuristic.selector.move.generic.ChangeMoveSelectorConfig;
import ai.timefold.solver.core.config.localsearch.LocalSearchPhaseConfig;
import ai.timefold.solver.core.config.localsearch.decider.acceptor.AcceptorType;
import ai.timefold.solver.core.config.localsearch.decider.acceptor.LocalSearchAcceptorConfig;
import ai.timefold.solver.core.config.localsearch.decider.forager.LocalSearchForagerConfig;
import ai.timefold.solver.core.config.localsearch.decider.forager.LocalSearchPickEarlyType;
import ai.timefold.solver.core.config.solver.EnvironmentMode;
import ai.timefold.solver.core.config.solver.SolverConfig;
import ai.timefold.solver.core.config.solver.termination.TerminationConfig;

public final class HeuristicBenchmark {
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
        if (args.length != 4) throw new IllegalArgumentException("expected hill|tabu|step|deluge steps warmup samples");
        String algorithm = args[0];
        int steps = Integer.parseInt(args[1]), warmup = Integer.parseInt(args[2]), samples = Integer.parseInt(args[3]);
        if (steps < 1 || warmup < 0 || samples < 1) throw new IllegalArgumentException("invalid counts");
        var acceptor = switch (algorithm) {
            case "hill" -> new LocalSearchAcceptorConfig().withAcceptorTypeList(List.of(AcceptorType.HILL_CLIMBING));
            case "step" -> new LocalSearchAcceptorConfig().withStepCountingHillClimbingSize(7)
                .withStepCountingHillClimbingType(ai.timefold.solver.core.config.localsearch.decider.acceptor.stepcountinghillclimbing.StepCountingHillClimbingType.STEP);
            case "deluge" -> {
                var rain = new LocalSearchAcceptorConfig();
                rain.setGreatDelugeWaterLevelIncrementScore("0hard/1soft");
                yield rain;
            }
            case "tabu" -> new LocalSearchAcceptorConfig().withEntityTabuSize(2);
            default -> throw new IllegalArgumentException("unknown algorithm");
        };
        var phase = new LocalSearchPhaseConfig()
            .withMoveSelectorConfig(new ChangeMoveSelectorConfig().withSelectionOrder(SelectionOrder.ORIGINAL))
            .withAcceptorConfig(acceptor)
            .withForagerConfig(new LocalSearchForagerConfig().withAcceptedCountLimit(Integer.MAX_VALUE)
                .withPickEarlyType(LocalSearchPickEarlyType.NEVER).withBreakTieRandomly(false))
            .withTerminationConfig(new TerminationConfig().withStepCountLimit(steps));
        var factory = SolverFactory.<SolverOracle.Problem>create(new SolverConfig()
            .withSolutionClass(SolverOracle.Problem.class).withEntityClasses(SolverOracle.Job.class)
            .withEasyScoreCalculatorClass(CountingCalculator.class).withRandomSeed(20261001L)
            .withEnvironmentMode(EnvironmentMode.NO_ASSERT).withPhases(phase));
        try (var input = new BufferedReader(new InputStreamReader(System.in))) {
            for (String line; (line = input.readLine()) != null;) {
                String[] fields = line.split("\t", -1);
                var original = SolverOracle.parse(fields);
                String[] values = fields[2].split(",");
                for (int i = 0; i < values.length; i++) {
                    int value = Integer.parseInt(values[i]);
                    if (value < 0 || value >= original.values.size()) throw new IllegalArgumentException("invalid initial value");
                    original.jobs.get(i).value = value;
                }
                for (int run = 0; run < warmup + samples; run++) {
                    CountingCalculator.calls = 0;
                    long started = System.nanoTime();
                    var solved = factory.buildSolver().solve(copy(original));
                    long calls = CountingCalculator.calls;
                    if (!new SolverOracle.Calculator().calculateScore(solved).equals(solved.score))
                        throw new IllegalStateException("incorrect score");
                    for (int i = 0; i < solved.jobs.size(); i++) {
                        var value = solved.jobs.get(i).value;
                        if (value == null || value < 0 || value >= original.values.size()
                            || (original.jobs.get(i).pinned && !value.equals(original.jobs.get(i).value)))
                            throw new IllegalStateException("invalid result");
                    }
                    long elapsed = System.nanoTime() - started;
                    if (run >= warmup) {
                        String assignment = String.join(",", solved.jobs.stream().map(j -> j.value.toString()).toList());
                        System.out.println(fields[0] + "\t" + elapsed + "\t" + solved.score.hardScore()
                            + "\t" + solved.score.softScore() + "\t" + assignment + "\t" + calls);
                    }
                }
            }
        }
    }
}
