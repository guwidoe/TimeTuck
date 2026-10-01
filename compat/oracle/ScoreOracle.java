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
    public static void main(String[] args) throws Exception {
        var reader = new BufferedReader(new InputStreamReader(System.in));
        for (String line; (line = reader.readLine()) != null;) {
            try {
                String[] f = line.split("\t", -1);
                System.out.println(switch(f[0]) {
                    case "simple" -> run(SimpleScore.parseScore(f[2]), f, SimpleScore::parseScore);
                    case "hard_soft" -> run(HardSoftScore.parseScore(f[2]), f, HardSoftScore::parseScore);
                    case "hard_medium_soft" -> run(HardMediumSoftScore.parseScore(f[2]), f, HardMediumSoftScore::parseScore);
                    default -> throw new IllegalArgumentException("type");
                });
            } catch (IllegalArgumentException e) { System.out.println("ERROR"); }
        }
    }
}
