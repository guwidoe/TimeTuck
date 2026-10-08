// Original benchmark adapter using only Timefold Community public configuration APIs.
import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.util.List;
import ai.timefold.solver.core.api.solver.SolverFactory;
import ai.timefold.solver.core.config.heuristic.selector.common.SelectionOrder;
import ai.timefold.solver.core.config.heuristic.selector.move.generic.ChangeMoveSelectorConfig;
import ai.timefold.solver.core.config.heuristic.selector.move.generic.SwapMoveSelectorConfig;
import ai.timefold.solver.core.config.localsearch.LocalSearchPhaseConfig;
import ai.timefold.solver.core.config.localsearch.decider.acceptor.AcceptorType;
import ai.timefold.solver.core.config.localsearch.decider.acceptor.LocalSearchAcceptorConfig;
import ai.timefold.solver.core.config.localsearch.decider.forager.LocalSearchForagerConfig;
import ai.timefold.solver.core.config.localsearch.decider.forager.LocalSearchPickEarlyType;
import ai.timefold.solver.core.config.solver.EnvironmentMode;
import ai.timefold.solver.core.config.solver.termination.TerminationConfig;

public final class StreamBenchmark {
    public static void main(String[] args) throws Exception {
        if (args.length < 4 || args.length > 5) throw new IllegalArgumentException("expected hill|tabu steps warmup samples [change|swap]");
        String neighborhood = args.length == 5 ? args[4] : "change";
        var selector = switch (neighborhood) {
            case "change" -> new ChangeMoveSelectorConfig().withSelectionOrder(SelectionOrder.ORIGINAL);
            case "swap" -> new SwapMoveSelectorConfig().withSelectionOrder(SelectionOrder.ORIGINAL);
            default -> throw new IllegalArgumentException("unknown neighborhood");
        };
        String algorithm = args[0];
        int steps = Integer.parseInt(args[1]), warmup = Integer.parseInt(args[2]), samples = Integer.parseInt(args[3]);
        if (steps < 1 || warmup < 0 || samples < 1) throw new IllegalArgumentException("invalid counts");
        var acceptor = switch (algorithm) {
            case "hill" -> new LocalSearchAcceptorConfig().withAcceptorTypeList(List.of(AcceptorType.HILL_CLIMBING));
            case "tabu" -> new LocalSearchAcceptorConfig().withEntityTabuSize(2);
            default -> throw new IllegalArgumentException("unknown algorithm");
        };
        var phase = new LocalSearchPhaseConfig()
            .withMoveSelectorConfig(selector)
            .withAcceptorConfig(acceptor)
            .withForagerConfig(new LocalSearchForagerConfig().withAcceptedCountLimit(Integer.MAX_VALUE)
                .withPickEarlyType(LocalSearchPickEarlyType.NEVER).withBreakTieRandomly(false))
            .withTerminationConfig(new TerminationConfig().withStepCountLimit(steps));
        var factory = SolverFactory.<StreamOracle.Problem>create(StreamOracle.config()
            .withEnvironmentMode(EnvironmentMode.NO_ASSERT).withPhases(phase));
        try (var input = new BufferedReader(new InputStreamReader(System.in))) {
            for (String line; (line = input.readLine()) != null;) {
                String[] fields = line.split("\t", -1);
                var original = StreamOracle.parse(fields);
                String[] values = fields[2].split(",");
                for (int i = 0; i < values.length; i++) {
                    int value = Integer.parseInt(values[i]);
                    if (value < 0 || value >= original.values.size()) throw new IllegalArgumentException("invalid initial value");
                    original.jobs.get(i).value = value;
                }
                for (int run = 0; run < warmup + samples; run++) {
                    StreamOracle.Provider.calls = 0;
                    long started = System.nanoTime();
                    var solved = factory.buildSolver().solve(StreamOracle.copy(original));
                    long calls = StreamOracle.Provider.calls;
                    if (!StreamOracle.independentScore(solved).equals(solved.score))
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
