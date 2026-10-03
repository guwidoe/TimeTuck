// Original public-API adapter; no Timefold production source is incorporated.
import ai.timefold.solver.core.api.score.*;
import java.io.*;
import java.math.BigDecimal;
import java.util.*;
public class DecimalScoreOracle {
 @SuppressWarnings({"rawtypes", "unchecked"})
 static String run(String line) {
  try {
   String[] p=line.split("\t",-1);
   Score a=parse(p[0],p[2]);
   if(p[1].equals("cmp")) {Score b=parse(p[0],p[3]);return Integer.signum(a.compareTo(b))+"|"+a.equals(b);}
   Score r=switch(p[1]) {
    case "parse" -> a;
    case "add" -> a.add(parse(p[0],p[3]));
    case "sub" -> a.subtract(parse(p[0],p[3]));
    case "neg" -> a.negate();
    case "abs" -> a.abs();
    case "mul" -> a.multiply(Double.parseDouble(p[3]));
    case "div" -> a.divide(Double.parseDouble(p[3]));
    case "pow" -> a.power(Double.parseDouble(p[3]));
    default -> throw new IllegalArgumentException();
   };
   var levels=new ArrayList<String>();
   for(Number n:r.toLevelNumbers()) {var d=(BigDecimal)n;levels.add(d.unscaledValue()+"@"+d.scale());}
   return r.structuralScore()+"|"+r+"|"+r.toShortString()+"|"+r.isFeasible()+"|"+String.join(",",levels);
  } catch(Exception e) {return "ERROR";}
 }
 @SuppressWarnings("rawtypes")
 static Score parse(String type,String text) {
  return switch(type) {case "simple" -> SimpleBigDecimalScore.parseScore(text);
   case "hard_soft" -> HardSoftBigDecimalScore.parseScore(text);
   case "hard_medium_soft" -> HardMediumSoftBigDecimalScore.parseScore(text);
   default -> throw new IllegalArgumentException();};
 }
 public static void main(String[] args) throws Exception {
  try(var reader=new BufferedReader(new InputStreamReader(System.in))) {
   for(String line; (line=reader.readLine())!=null;) System.out.println(run(line));
  }
 }
}
