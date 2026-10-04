// Original public composed-collector comparison. No upstream-method credit.
import java.io.*;
import java.util.*;
import ai.timefold.solver.core.api.score.stream.ConstraintCollectors;
import ai.timefold.solver.core.api.score.stream.uni.*;
public class CompositionCollectorOracle {
 static String value(Long v){return v==null?"-":v.toString();}
 static String average(Double v){return v==null?"-":Long.toUnsignedString(Double.doubleToRawLongBits(v));}
 static <C> void run(String[] f,UniConstraintCollector<Long,C,String> collector){
  var box=collector.supplier().get();var acc=(UniConstraintCollectorAccumulator<C,Long>)collector.accumulator();var items=Arrays.stream(f[2].split(",")).map(Long::valueOf).toList();
  var handles=new HashMap<Integer,UniConstraintCollectorValueHandle<Long>>();var slots=new HashMap<Integer,Integer>();
  java.util.function.BiConsumer<Integer,Integer> put=(slot,item)->{if(item<0){var h=handles.remove(slot);if(h!=null)h.remove();slots.remove(slot);}else{var h=handles.get(slot);if(h==null){h=acc.intoGroup(box);h.add(items.get(item));handles.put(slot,h);}else h.replaceWith(items.get(item));slots.put(slot,item);}};
  int state=0;System.out.println(f[0]+"\t"+state+++"\t"+collector.finisher().apply(box));
  for(var op:f[3].split(";")){if(op.isEmpty())continue;var v=op.split(":");int slot=Integer.parseInt(v[1]),item=Integer.parseInt(v[2]),old=slots.getOrDefault(slot,-1);put.accept(slot,item);System.out.println(f[0]+"\t"+state+++"\t"+collector.finisher().apply(box));if(v[0].equals("t")){put.accept(slot,old);System.out.println(f[0]+"\t"+state+++"\t"+collector.finisher().apply(box));}}
 }
 public static void main(String[] args)throws Exception{try(var in=new BufferedReader(new InputStreamReader(System.in))){String line;while((line=in.readLine())!=null){var f=line.split("\t",-1);switch(f[1]){
  case "two" -> run(f,ConstraintCollectors.compose(ConstraintCollectors.<Long>min(),ConstraintCollectors.<Long>max(),(a,b)->value(a)+","+value(b)));
  case "three" -> run(f,ConstraintCollectors.compose(ConstraintCollectors.<Long>min(),ConstraintCollectors.<Long>max(),ConstraintCollectors.average((Long v)->v),(a,b,c)->value(a)+","+value(b)+","+average(c)));
  case "four" -> run(f,ConstraintCollectors.compose(ConstraintCollectors.<Long>count(),ConstraintCollectors.<Long>min(),ConstraintCollectors.<Long>max(),ConstraintCollectors.average((Long v)->v),(n,a,b,c)->n+","+value(a)+","+value(b)+","+average(c)));
  case "then" -> run(f,ConstraintCollectors.collectAndThen(ConstraintCollectors.<Long>count(),n->Long.toString(n*10)));
  default -> throw new IllegalArgumentException("unknown mode");
 }}}}
}
