// Original development probe; only pinned Community public APIs are used.
import ai.timefold.solver.core.api.score.HardSoftScore;
import ai.timefold.solver.core.api.score.stream.*;
import ai.timefold.solver.core.api.solver.*;

public final class AggregationBoundaryOracle {
    public static class Provider implements ConstraintProvider {
        @Override public Constraint[] defineConstraints(ConstraintFactory factory) {
            // Constant penalties observe row presence even when the collected sum is zero.
            return new Constraint[]{
                factory.forEach(StreamOracle.Job.class)
                    .groupBy(ConstraintCollectors.count())
                    .penalize(HardSoftScore.ONE_SOFT, count -> 1L).asConstraint("count-presence"),
                factory.forEach(StreamOracle.Job.class)
                    .groupBy(ConstraintCollectors.sum((StreamOracle.Job j) -> j.weights[j.value]))
                    .penalize(HardSoftScore.ONE_SOFT, sum -> 1L).asConstraint("sum-presence")
            };
        }
    }
    static void check(SolutionManager<StreamOracle.Problem, HardSoftScore> manager,
            StreamOracle.Problem problem, long expectedSoft) {
        var score = manager.update(problem, SolutionUpdatePolicy.UPDATE_SCORE_ONLY);
        if (score.hardScore() != 0 || score.softScore() != expectedSoft)
            throw new IllegalStateException("Unexpected aggregation score: " + score);
    }
    public static void main(String[] args) {
        var manager = SolutionManager.<StreamOracle.Problem, HardSoftScore>create(
                SolverFactory.create(StreamOracle.config().withConstraintProviderClass(Provider.class)));
        var problem = StreamOracle.parse(new String[]{"boundary", "1", "0", "0", "", "0"});
        var job = problem.jobs.remove(0);
        check(manager, problem, 0);
        problem.jobs.add(job);
        check(manager, problem, -2);
        problem.jobs.clear();
        check(manager, problem, 0);
        System.out.println("Aggregation boundaries: 3/3 states");
    }
}
