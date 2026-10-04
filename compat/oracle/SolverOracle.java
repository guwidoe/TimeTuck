// Original development adapter. Uses only Timefold Community public APIs.
import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import ai.timefold.solver.core.api.domain.entity.PlanningEntity;
import ai.timefold.solver.core.api.domain.entity.PlanningPin;
import ai.timefold.solver.core.api.domain.solution.PlanningEntityCollectionProperty;
import ai.timefold.solver.core.api.domain.solution.PlanningScore;
import ai.timefold.solver.core.api.domain.solution.PlanningSolution;
import ai.timefold.solver.core.api.domain.solution.ProblemFactCollectionProperty;
import ai.timefold.solver.core.api.domain.valuerange.ValueRangeProvider;
import ai.timefold.solver.core.api.domain.variable.PlanningVariable;
import ai.timefold.solver.core.api.score.HardSoftScore;
import ai.timefold.solver.core.api.score.calculator.EasyScoreCalculator;
import ai.timefold.solver.core.api.solver.SolverFactory;
import ai.timefold.solver.core.config.exhaustivesearch.ExhaustiveSearchPhaseConfig;
import ai.timefold.solver.core.config.exhaustivesearch.ExhaustiveSearchType;
import ai.timefold.solver.core.config.exhaustivesearch.NodeExplorationType;
import ai.timefold.solver.core.config.heuristic.selector.entity.EntitySorterManner;
import ai.timefold.solver.core.config.heuristic.selector.value.ValueSorterManner;
import ai.timefold.solver.core.config.solver.SolverConfig;

public final class SolverOracle {
    @PlanningEntity
    public static class Job {
        @PlanningVariable(valueRangeProviderRefs = "values")
        public Integer value;
        @PlanningPin
        public boolean pinned;
        public Job() {}
        Job(Integer value, boolean pinned) { this.value = value; this.pinned = pinned; }
    }

    @PlanningSolution
    public static class Problem {
        @ValueRangeProvider(id = "values") @ProblemFactCollectionProperty
        public List<Integer> values;
        @PlanningEntityCollectionProperty
        public List<Job> jobs;
        @PlanningScore
        public HardSoftScore score;
        public int[][] edges;
        public long[][] weights;
        public Problem() {}
    }

    public static class Calculator implements EasyScoreCalculator<Problem, HardSoftScore> {
        @Override public HardSoftScore calculateScore(Problem problem) {
            long hard = 0, soft = 0;
            for (int[] edge : problem.edges) {
                Integer a = problem.jobs.get(edge[0]).value, b = problem.jobs.get(edge[1]).value;
                if (a != null && a.equals(b)) hard--;
            }
            for (int i = 0; i < problem.jobs.size(); i++) {
                Integer value = problem.jobs.get(i).value;
                if (value != null) soft += problem.weights[i][value];
            }
            return HardSoftScore.of(hard, soft);
        }
    }

    static Problem parse(String[] fields) {
        if (fields.length != 6) throw new IllegalArgumentException("Expected six legacy fields");
        int count = Integer.parseInt(fields[1]);
        var problem = new Problem();
        problem.values = new ArrayList<>();
        for (int v = 0; v < count; v++) problem.values.add(v);
        String[] initial = fields[2].split(",");
        problem.jobs = new ArrayList<>();
        for (int i = 0; i < initial.length; i++) {
            boolean pinned = fields[3].charAt(i) == '1';
            // Normalize to a fresh exhaustive problem: only pins retain values.
            problem.jobs.add(new Job(!pinned || initial[i].equals("-") ? null : Integer.valueOf(initial[i]), pinned));
        }
        problem.edges = fields[4].isEmpty() ? new int[0][] : Arrays.stream(fields[4].split(","))
            .map(edge -> Arrays.stream(edge.split(":")).mapToInt(Integer::parseInt).toArray()).toArray(int[][]::new);
        String[] weights = fields[5].split(",");
        problem.weights = new long[initial.length][count];
        for (int i = 0; i < initial.length; i++) for (int v = 0; v < count; v++) {
            problem.weights[i][v] = Long.parseLong(weights[i * count + v]);
        }
        return problem;
    }

    static SolverFactory<Problem> factory() {
        var config = new SolverConfig().withSolutionClass(Problem.class).withEntityClasses(Job.class)
            .withEasyScoreCalculatorClass(Calculator.class).withRandomSeed(20261001L)
            .withPhases(new ExhaustiveSearchPhaseConfig().withExhaustiveSearchType(ExhaustiveSearchType.BRUTE_FORCE)
                .withNodeExplorationType(NodeExplorationType.ORIGINAL_ORDER)
                .withEntitySorterManner(EntitySorterManner.NONE).withValueSorterManner(ValueSorterManner.NONE));
        return SolverFactory.<Problem>create(config);
    }

    static SolverFactory<Problem> branchFactory(String algorithm) {
        return branchFactory(algorithm, "ONLY_DOWN");
    }

    static SolverFactory<Problem> branchFactory(String algorithm, String trend) {
        var order = switch (algorithm) {
            case "depth" -> NodeExplorationType.DEPTH_FIRST;
            case "breadth" -> NodeExplorationType.BREADTH_FIRST;
            case "score" -> NodeExplorationType.SCORE_FIRST;
            case "bound" -> NodeExplorationType.OPTIMISTIC_BOUND_FIRST;
            default -> throw new IllegalArgumentException("Unknown branch exploration");
        };
        var config = new SolverConfig().withSolutionClass(Problem.class).withEntityClasses(Job.class)
            .withScoreDirectorFactory(new ai.timefold.solver.core.config.score.director.ScoreDirectorFactoryConfig()
                .withEasyScoreCalculatorClass(Calculator.class).withInitializingScoreTrend(trend))
            .withRandomSeed(20261001L)
            .withPhases(new ExhaustiveSearchPhaseConfig().withExhaustiveSearchType(ExhaustiveSearchType.BRANCH_AND_BOUND)
                .withNodeExplorationType(order).withEntitySorterManner(EntitySorterManner.NONE)
                .withValueSorterManner(ValueSorterManner.NONE));
        return SolverFactory.<Problem>create(config);
    }

    public static void main(String[] args) throws Exception {
        if (args.length != 0 && args.length != 2) throw new IllegalArgumentException("Expected order and trend, or no arguments");
        var factory = args.length == 0 ? factory() : branchFactory(args[0], args[1]);
        try (var input = new BufferedReader(new InputStreamReader(System.in))) {
            for (String line; (line = input.readLine()) != null;) {
                String[] fields = line.split("\t", -1);
                if (fields.length == 9) {
                    if (args.length != 0) throw new IllegalArgumentException("Trend adapter requires homogeneous required domains");
                    System.out.println(RangeSolverOracle.solve(fields));
                    continue;
                }
                var solved = factory.buildSolver().solve(parse(fields));
                var recomputed = new Calculator().calculateScore(solved);
                if (!recomputed.equals(solved.score)) throw new IllegalStateException("Returned score failed recomputation");
                String values = String.join(",", solved.jobs.stream().map(j -> j.value == null ? "-" : j.value.toString()).toList());
                System.out.println(fields[0] + "\t" + solved.score.hardScore() + "\t" + solved.score.softScore() + "\t" + values);
            }
        }
    }
}
