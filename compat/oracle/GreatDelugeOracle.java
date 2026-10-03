// Original development adapter; only public Timefold Community configuration APIs.
import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.util.Arrays;
import ai.timefold.solver.core.api.solver.SolverFactory;
import ai.timefold.solver.core.config.heuristic.selector.common.SelectionOrder;
import ai.timefold.solver.core.config.heuristic.selector.move.generic.ChangeMoveSelectorConfig;
import ai.timefold.solver.core.config.localsearch.LocalSearchPhaseConfig;
import ai.timefold.solver.core.config.localsearch.decider.acceptor.LocalSearchAcceptorConfig;
import ai.timefold.solver.core.config.localsearch.decider.forager.LocalSearchForagerConfig;
import ai.timefold.solver.core.config.localsearch.decider.forager.LocalSearchPickEarlyType;
import ai.timefold.solver.core.config.solver.EnvironmentMode;
import ai.timefold.solver.core.config.solver.SolverConfig;
import ai.timefold.solver.core.config.solver.termination.TerminationConfig;

public final class GreatDelugeOracle {
    public static void main(String[] args) throws Exception {
        try (var input = new BufferedReader(new InputStreamReader(System.in))) {
            for (String line; (line = input.readLine()) != null;) {
                var fields = line.split("\t", -1);
                if (fields.length != 11) throw new IllegalArgumentException("expected eleven fields");
                var original = SolverOracle.parse(Arrays.copyOf(fields, 6));
                var initial = fields[2].split(",");
                for (int i = 0; i < initial.length; i++) {
                    int v = Integer.parseInt(initial[i]);
                    if (v < 0 || v >= original.values.size()) throw new IllegalArgumentException("initial value");
                    original.jobs.get(i).value = v;
                }
                int steps = Integer.parseInt(fields[9]);
                if (steps < 0) throw new IllegalArgumentException("invalid steps");
                var acceptor = new LocalSearchAcceptorConfig();
                switch (fields[6]) {
                    case "fixed" -> acceptor.setGreatDelugeWaterLevelIncrementScore(fields[7]+"hard/"+fields[8]+"soft");
                    case "ratio" -> acceptor.setGreatDelugeWaterLevelIncrementRatio(Double.parseDouble(fields[7])/Double.parseDouble(fields[8]));
                    default -> throw new IllegalArgumentException("invalid rain");
                }
                int selected = switch (fields[10]) {
                    case "first" -> 1;
                    case "best" -> Integer.MAX_VALUE;
                    default -> throw new IllegalArgumentException("invalid selection");
                };
                var termination = new TerminationConfig().withStepCountLimit(steps);
                var phase = new LocalSearchPhaseConfig()
                    .withMoveSelectorConfig(new ChangeMoveSelectorConfig().withSelectionOrder(SelectionOrder.ORIGINAL))
                    .withAcceptorConfig(acceptor)
                    .withForagerConfig(new LocalSearchForagerConfig().withAcceptedCountLimit(selected)
                        .withPickEarlyType(LocalSearchPickEarlyType.NEVER).withBreakTieRandomly(false))
                    .withTerminationConfig(termination);
                var factory = SolverFactory.<SolverOracle.Problem>create(new SolverConfig()
                    .withSolutionClass(SolverOracle.Problem.class).withEntityClasses(SolverOracle.Job.class)
                    .withEasyScoreCalculatorClass(SolverOracle.Calculator.class).withRandomSeed(20261220L)
                    .withEnvironmentMode(EnvironmentMode.NO_ASSERT).withPhases(phase));
                var solved = factory.buildSolver().solve(HeuristicBenchmark.copy(original));
                if (!new SolverOracle.Calculator().calculateScore(solved).equals(solved.score))
                    throw new IllegalStateException("incorrect returned score");
                String values = String.join(",", solved.jobs.stream().map(j -> j.value.toString()).toList());
                System.out.println(fields[0] + "\t" + solved.score.hardScore() + "\t" + solved.score.softScore() + "\t" + values);
            }
        }
    }
}
