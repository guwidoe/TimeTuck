// Original full-scoring construction benchmark. Public Community APIs only.
import java.io.BufferedReader;
import java.io.InputStreamReader;
import ai.timefold.solver.core.api.score.HardSoftScore;
import ai.timefold.solver.core.api.solver.SolverFactory;
import ai.timefold.solver.core.config.solver.SolverConfig;
import ai.timefold.solver.core.config.constructionheuristic.ConstructionHeuristicPhaseConfig;
import ai.timefold.solver.core.config.constructionheuristic.ConstructionHeuristicType;
import ai.timefold.solver.core.config.constructionheuristic.decider.forager.ConstructionHeuristicForagerConfig;
import ai.timefold.solver.core.config.constructionheuristic.decider.forager.ConstructionHeuristicPickEarlyType;
import ai.timefold.solver.core.config.heuristic.selector.entity.EntitySorterManner;
import ai.timefold.solver.core.config.heuristic.selector.value.ValueSorterManner;

public final class ConstructionBenchmark {
    public static class CountingCalculator extends SolverOracle.Calculator {
        static long calls;
        @Override public HardSoftScore calculateScore(SolverOracle.Problem problem) {
            calls++;
            return super.calculateScore(problem);
        }
    }
    public static void main(String[] args) throws Exception {
        if (args.length != 3) throw new IllegalArgumentException("Expected mode warmup samples");
        var type = switch (args[0]) {
            case "sequential" -> ConstructionHeuristicType.ALLOCATE_ENTITY_FROM_QUEUE;
            case "pooled" -> ConstructionHeuristicType.CHEAPEST_INSERTION;
            case "value-queue" -> ConstructionHeuristicType.ALLOCATE_TO_VALUE_FROM_QUEUE;
            default -> throw new IllegalArgumentException("Unknown construction mode");
        };
        int warmup = Integer.parseInt(args[1]), samples = Integer.parseInt(args[2]);
        if (warmup < 0 || samples < 1) throw new IllegalArgumentException("Invalid sample counts");
        int count = Math.addExact(warmup, samples);
        var config = new SolverConfig().withSolutionClass(SolverOracle.Problem.class)
            .withEntityClasses(SolverOracle.Job.class).withEasyScoreCalculatorClass(CountingCalculator.class)
            .withRandomSeed(20261001L)
            .withPhases(new ConstructionHeuristicPhaseConfig().withConstructionHeuristicType(type)
                .withEntitySorterManner(EntitySorterManner.NONE).withValueSorterManner(ValueSorterManner.NONE)
                .withForagerConfig(new ConstructionHeuristicForagerConfig()
                    .withPickEarlyType(ConstructionHeuristicPickEarlyType.NEVER)));
        var factory = SolverFactory.<SolverOracle.Problem>create(config);
        var calculator = new SolverOracle.Calculator();
        try (var input = new BufferedReader(new InputStreamReader(System.in))) {
            for (String line; (line = input.readLine()) != null;) {
                var fields = line.split("\t", -1);
                var problem = SolverOracle.parse(fields);
                for (int sample = 0; sample < count; sample++) {
                    CountingCalculator.calls = 0;
                    long started = System.nanoTime();
                    var result = factory.buildSolver().solve(problem);
                    if (!calculator.calculateScore(result).equals(result.score)) throw new IllegalStateException("Incorrect score");
                    for (int i = 0; i < result.jobs.size(); i++) {
                        var value = result.jobs.get(i).value;
                        if (value == null || !problem.values.contains(value)) throw new IllegalStateException("Invalid assignment");
                        if (problem.jobs.get(i).pinned && !value.equals(problem.jobs.get(i).value)) throw new IllegalStateException("Pin changed");
                    }
                    long elapsed = System.nanoTime() - started;
                    if (sample >= warmup) {
                        String values = String.join(",", result.jobs.stream().map(j -> j.value.toString()).toList());
                        System.out.println(fields[0] + "\t" + elapsed + "\t" + result.score.hardScore() + "\t" + result.score.softScore() + "\t" + values + "\t" + CountingCalculator.calls);
                    }
                }
            }
        }
    }
}
