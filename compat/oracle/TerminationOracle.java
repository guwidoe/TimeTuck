// Original development adapter; only public Timefold Community configuration APIs.
import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.util.Arrays;
import java.util.List;
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
import ai.timefold.solver.core.config.solver.termination.TerminationCompositionStyle;

public final class TerminationOracle {
    public static void main(String[] args) throws Exception {
        try (var input = new BufferedReader(new InputStreamReader(System.in))) {
            for (String line; (line = input.readLine()) != null;) {
                var fields = line.split("\t", -1);
                if (fields.length != 10) throw new IllegalArgumentException("expected ten fields");
                var original = SolverOracle.parse(Arrays.copyOf(fields, 6));
                var initial = fields[2].split(",");
                for (int i = 0; i < initial.length; i++) {
                    int v = Integer.parseInt(initial[i]);
                    if (v < 0 || v >= original.values.size()) throw new IllegalArgumentException("initial value");
                    original.jobs.get(i).value = v;
                }
                var termination = new TerminationConfig()
                    .withTerminationCompositionStyle(TerminationCompositionStyle.OR)
                    .withStepCountLimit(Integer.parseInt(fields[6]));
                if (!fields[7].equals("-")) termination.withUnimprovedStepCountLimit(Integer.parseInt(fields[7]));
                if (!fields[8].equals("-")) termination.withBestScoreLimit(fields[8]);
                if (fields[9].equals("1")) termination.withBestScoreFeasible(true);
                else if (!fields[9].equals("0")) throw new IllegalArgumentException("feasibility flag");
                var phase = new LocalSearchPhaseConfig()
                    .withMoveSelectorConfig(new ChangeMoveSelectorConfig().withSelectionOrder(SelectionOrder.ORIGINAL))
                    .withAcceptorConfig(new LocalSearchAcceptorConfig().withAcceptorTypeList(List.of(AcceptorType.HILL_CLIMBING)))
                    .withForagerConfig(new LocalSearchForagerConfig().withAcceptedCountLimit(Integer.MAX_VALUE)
                        .withPickEarlyType(LocalSearchPickEarlyType.NEVER).withBreakTieRandomly(false))
                    .withTerminationConfig(termination);
                var factory = SolverFactory.<SolverOracle.Problem>create(new SolverConfig()
                    .withSolutionClass(SolverOracle.Problem.class).withEntityClasses(SolverOracle.Job.class)
                    .withEasyScoreCalculatorClass(SolverOracle.Calculator.class).withRandomSeed(20261119L)
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
