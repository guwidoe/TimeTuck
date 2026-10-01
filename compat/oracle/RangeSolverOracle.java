// Original development adapter. Public Community APIs only; no production code port.
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

public final class RangeSolverOracle {
    @PlanningEntity
    public static class Job {
        public int id;
        public boolean optional;
        public List<Integer> range;
        @PlanningVariable(valueRangeProviderRefs = "requiredRange") public Integer requiredValue;
        @PlanningVariable(valueRangeProviderRefs = "optionalRange", allowsUnassigned = true) public Integer optionalValue;
        @PlanningPin public boolean pinned;
        public Job() {}
        // The inactive variable has exactly one complete choice. This supports
        // mixed required/optional inputs using one entity class in exhaustive search.
        @ValueRangeProvider(id = "requiredRange")
        public List<Integer> getRequiredRange() { return optional ? List.of(0) : range; }
        @ValueRangeProvider(id = "optionalRange")
        public List<Integer> getOptionalRange() { return optional ? range : List.of(); }
        Integer value() { return optional ? optionalValue : requiredValue; }
    }
    @PlanningSolution
    public static class Problem {
        @ProblemFactCollectionProperty public List<Integer> values;
        @PlanningEntityCollectionProperty public List<Job> jobs;
        @PlanningScore public HardSoftScore score;
        public long[][] weights;
        public long[] unassignedWeights;
        public int[][] edges;
        public Problem() {}
        Integer[] assignment() {
            var result = new Integer[weights.length];
            for (var job : jobs) result[job.id] = job.value();
            return result;
        }
    }
    public static class Calculator implements EasyScoreCalculator<Problem, HardSoftScore> {
        @Override public HardSoftScore calculateScore(Problem problem) {
            Integer[] values = problem.assignment();
            long hard = 0, soft = 0;
            for (var edge : problem.edges) {
                if (values[edge[0]] != null && values[edge[0]].equals(values[edge[1]])) hard--;
            }
            for (int i = 0; i < values.length; i++) {
                soft += values[i] == null ? problem.unassignedWeights[i] : problem.weights[i][values[i]];
            }
            return HardSoftScore.of(hard, soft);
        }
    }
    static Problem parse(String[] fields) {
        if (fields.length != 9) throw new IllegalArgumentException("Expected nine fields");
        int count = Integer.parseInt(fields[1]);
        String[] initial = fields[2].split(",", -1);
        if (count <= 0 || fields[3].length() != initial.length || !fields[3].matches("[01]+")) throw new IllegalArgumentException("Invalid size or pins");
        var legacy = SolverOracle.parse(Arrays.copyOf(fields, 6));
        String[] ranges = fields[6].split(";", -1);
        String optional = fields[7];
        String[] nullWeights = fields[8].split(",", -1);
        int n = legacy.jobs.size();
        if (ranges.length != n || optional.length() != n || nullWeights.length != n) {
            throw new IllegalArgumentException("Range/optional/unassigned-weight count mismatch");
        }
        var p = new Problem();
        p.values = legacy.values;
        p.weights = legacy.weights;
        p.edges = legacy.edges;
        p.jobs = new ArrayList<>();
        p.unassignedWeights = new long[n];
        for (int i = 0; i < n; i++) {
            List<Integer> range = ranges[i].isEmpty() ? List.of() : Arrays.stream(ranges[i].split(":", -1)).map(Integer::valueOf).toList();
            if (range.stream().distinct().count() != range.size() || !p.values.containsAll(range)) {
                throw new IllegalArgumentException("Invalid range");
            }
            if (!initial[i].equals("-") && !range.contains(Integer.valueOf(initial[i]))) throw new IllegalArgumentException("Initial value outside range");
            p.unassignedWeights[i] = Long.parseLong(nullWeights[i]);
            var original = legacy.jobs.get(i);
            if (original.value != null && !range.contains(original.value)) throw new IllegalArgumentException("Pin outside range");
            if (optional.charAt(i) != '0' && optional.charAt(i) != '1') throw new IllegalArgumentException("Invalid optional flag");
            var job = new Job(); job.id = i; job.range = range;
            job.optional = optional.charAt(i) == '1'; job.pinned = original.pinned;
            if (!job.optional && job.pinned && original.value == null) throw new IllegalArgumentException("Required pin unassigned");
            job.requiredValue = job.optional ? Integer.valueOf(0) : original.value;
            job.optionalValue = job.optional ? original.value : null;
            p.jobs.add(job);
        }
        return p;
    }
    private static final SolverFactory<Problem> FACTORY = SolverFactory.create(new SolverConfig()
        .withSolutionClass(Problem.class).withEntityClasses(Job.class)
        .withEasyScoreCalculatorClass(Calculator.class).withRandomSeed(20261001L)
        .withPhases(new ExhaustiveSearchPhaseConfig().withExhaustiveSearchType(ExhaustiveSearchType.BRUTE_FORCE)
            .withNodeExplorationType(NodeExplorationType.ORIGINAL_ORDER)
            .withEntitySorterManner(EntitySorterManner.NONE).withValueSorterManner(ValueSorterManner.NONE)));
    static String solve(String[] fields) {
        Problem original = parse(fields);
        Problem solved = FACTORY.buildSolver().solve(original);
        if (!new Calculator().calculateScore(solved).equals(solved.score)) throw new IllegalStateException("Score mismatch");
        for (var job : solved.jobs) {
            if (job.requiredValue == null || !job.getRequiredRange().contains(job.requiredValue)) throw new IllegalStateException("Invalid required value");
            if (job.optionalValue != null && !job.getOptionalRange().contains(job.optionalValue)) throw new IllegalStateException("Invalid optional value");
        }
        String values = String.join(",", Arrays.stream(solved.assignment()).map(v -> v == null ? "-" : v.toString()).toList());
        return fields[0] + "\t" + solved.score.hardScore() + "\t" + solved.score.softScore() + "\t" + values;
    }
}
