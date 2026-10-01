// Original matched score microbenchmark; no Timefold implementation copied.
import ai.timefold.solver.core.api.score.*;

public class ScoreBenchmark {
    static volatile long sink;
    static long run(String kind, long iterations, long seed) {
        long x = seed;
        switch (kind) {
            case "simple" -> {
                var score = SimpleScore.ZERO;
                for (long i=0; i<iterations; i++) {
                    x = x * 6364136223846793005L + 1442695040888963407L;
                    var delta = SimpleScore.of((x >> 32) % 101);
                    score = score.add(delta);
                    if (score.compareTo(SimpleScore.ZERO) > 0) score = score.subtract(delta);
                }
                return score.score() ^ x;
            }
            case "hard_soft" -> {
                var score = HardSoftScore.ZERO;
                for (long i=0; i<iterations; i++) {
                    x = x * 6364136223846793005L + 1442695040888963407L;
                    var delta = HardSoftScore.of((x >> 32) % 101, x % 101);
                    score = score.add(delta);
                    if (score.compareTo(HardSoftScore.ZERO) > 0) score = score.subtract(delta);
                }
                return score.hardScore() ^ score.softScore() ^ x;
            }
            case "hard_medium_soft" -> {
                var score = HardMediumSoftScore.ZERO;
                for (long i=0; i<iterations; i++) {
                    x = x * 6364136223846793005L + 1442695040888963407L;
                    var delta = HardMediumSoftScore.of((x >> 32) % 101, (x >> 16) % 101, x % 101);
                    score = score.add(delta);
                    if (score.compareTo(HardMediumSoftScore.ZERO) > 0) score = score.subtract(delta);
                }
                return score.hardScore() ^ score.mediumScore() ^ score.softScore() ^ x;
            }
            default -> throw new IllegalArgumentException("kind");
        }
    }
    public static void main(String[] args) {
        String kind=args[0]; long iterations=Long.parseLong(args[1]), seed=Long.parseLong(args[2]);
        int warmup=Integer.parseInt(args[3]), samples=Integer.parseInt(args[4]);
        for(int i=0; i<warmup; i++) sink=run(kind, iterations, seed);
        for(int i=0; i<samples; i++) {
            long start=System.nanoTime(); long checksum=run(kind, iterations, seed);
            long elapsed=System.nanoTime()-start; sink=checksum;
            System.out.println(elapsed+"\t"+checksum);
        }
    }
}
