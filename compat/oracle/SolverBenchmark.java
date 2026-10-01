// Original matched exact-search benchmark. Not derived from upstream benchmarks.
import java.io.BufferedReader;
import java.io.InputStreamReader;

public final class SolverBenchmark {
    public static void main(String[] args) throws Exception {
        int iterations = Integer.parseInt(args[0]), warmup = Integer.parseInt(args[1]), samples = Integer.parseInt(args[2]);
        var reader = new BufferedReader(new InputStreamReader(System.in));
        var problem = SolverOracle.parse(reader.readLine().split("\t", -1));
        var factory = SolverOracle.factory();
        var calculator = new SolverOracle.Calculator();
        for (int sample = -warmup; sample < samples; sample++) {
            long hard = 0, soft = 0;
            SolverOracle.Problem last = null;
            long started = System.nanoTime();
            for (int iteration = 0; iteration < iterations; iteration++) {
                last = factory.buildSolver().solve(problem);
                if (!calculator.calculateScore(last).equals(last.score)) throw new IllegalStateException("Incorrect returned score");
                for (int i = 0; i < last.jobs.size(); i++) {
                    var value = last.jobs.get(i).value;
                    if (value == null || !problem.values.contains(value)) throw new IllegalStateException("Incomplete/out-of-domain result");
                    if (problem.jobs.get(i).pinned && !value.equals(problem.jobs.get(i).value)) throw new IllegalStateException("Pin changed");
                }
                hard += last.score.hardScore(); soft += last.score.softScore();
            }
            long elapsed = System.nanoTime() - started;
            if (sample >= 0) {
                String values = String.join(",", last.jobs.stream().map(j -> j.value.toString()).toList());
                System.out.println(elapsed + "\t" + hard + "\t" + soft + "\t" + values);
            }
        }
    }
}
