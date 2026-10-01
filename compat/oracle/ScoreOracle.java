// Original development adapter. Uses only public Community score APIs.
import ai.timefold.solver.core.api.score.*;
import java.io.*;

public class ScoreOracle {
    private static <S extends Score<S>> String run(S a, String[] f, java.util.function.Function<String, S> parse) {
        if (f[1].equals("cmp")) return Integer.toString(Integer.signum(a.compareTo(parse.apply(f[3]))));
        S result = switch(f[1]) {
            case "parse" -> a;
            case "add" -> a.add(parse.apply(f[3]));
            case "sub" -> a.subtract(parse.apply(f[3]));
            case "neg" -> a.negate();
            case "abs" -> a.abs();
            case "mul" -> a.multiply(Double.parseDouble(f[3]));
            case "div" -> a.divide(Double.parseDouble(f[3]));
            case "pow" -> a.power(Double.parseDouble(f[3]));
            default -> throw new IllegalArgumentException("operation");
        };
        return result + "\t" + result.toShortString() + "\t" + result.isFeasible() + "\t" + result.structuralScore();
    }
    private static String bendable(String[] fields, int hard, int soft) {
        java.util.function.Function<String, BendableScore> parse = text -> {
            var score = BendableScore.parseScore(text);
            if (score.hardLevelsSize() != hard || score.softLevelsSize() != soft) {
                throw new IllegalArgumentException("Unexpected score dimensions");
            }
            return score;
        };
        return run(parse.apply(fields[2]), fields, parse);
    }
    public static void main(String[] args) throws Exception {
        var reader = new BufferedReader(new InputStreamReader(System.in));
        for (String line; (line = reader.readLine()) != null;) {
            try {
                String[] f = line.split("\t", -1);
                System.out.println(switch(f[0]) {
                    case "bendable_1_2" -> bendable(f, 1, 2);
                    case "bendable_2_3" -> bendable(f, 2, 3);
                    case "bendable_0_0" -> bendable(f, 0, 0);
                    case "bendable_0_2" -> bendable(f, 0, 2);
                    case "bendable_2_0" -> bendable(f, 2, 0);
                    case "simple" -> run(SimpleScore.parseScore(f[2]), f, SimpleScore::parseScore);
                    case "hard_soft" -> run(HardSoftScore.parseScore(f[2]), f, HardSoftScore::parseScore);
                    case "hard_medium_soft" -> run(HardMediumSoftScore.parseScore(f[2]), f, HardMediumSoftScore::parseScore);
                    default -> throw new IllegalArgumentException("type");
                });
            } catch (RuntimeException e) { System.out.println("ERROR"); }
        }
    }
}
