// Original public Community period-collector comparison; no source-method credit.
import java.io.*;
import java.time.Period;
import java.util.*;
import ai.timefold.solver.core.api.score.stream.ConstraintCollectors;
import ai.timefold.solver.core.api.score.stream.uni.*;
public class PeriodCollectorOracle {
 static void emit(String id,int state,Period p){System.out.println(id+"\t"+state+"\t"+p.getYears()+","+p.getMonths()+","+p.getDays());}
 static <C> void run(String[] f,UniConstraintCollector<Period,C,Period> collector){
  var box=collector.supplier().get();var acc=(UniConstraintCollectorAccumulator<C,Period>)collector.accumulator();var items=new ArrayList<Period>();
  for(var row:f[1].split(";")){var v=row.split(",");items.add(Period.of(Integer.parseInt(v[0]),Integer.parseInt(v[1]),Integer.parseInt(v[2])));}
  var slots=new HashMap<Integer,Integer>();var handles=new HashMap<Integer,UniConstraintCollectorValueHandle<Period>>();
  java.util.function.BiConsumer<Integer,Integer> put=(slot,item)->{if(item<0){var h=handles.remove(slot);if(h!=null)h.remove();slots.remove(slot);}else{var h=handles.get(slot);if(h==null){h=acc.intoGroup(box);h.add(items.get(item));handles.put(slot,h);}else h.replaceWith(items.get(item));slots.put(slot,item);}};
  int state=0;emit(f[0],state++,collector.finisher().apply(box));
  for(var op:f[2].split(";")){if(op.isEmpty())continue;var a=op.split(":");int slot=Integer.parseInt(a[1]),item=Integer.parseInt(a[2]),old=slots.getOrDefault(slot,-1);
   try{put.accept(slot,item);emit(f[0],state++,collector.finisher().apply(box));if(a[0].equals("t")){put.accept(slot,old);emit(f[0],state++,collector.finisher().apply(box));}}
   catch(ArithmeticException e){System.out.println(f[0]+"\t"+state+"\tOVERFLOW");return;}
  }
 }
 public static void main(String[] args)throws Exception{try(var in=new BufferedReader(new InputStreamReader(System.in))){String line;while((line=in.readLine())!=null){var f=line.split("\t",-1);run(f,ConstraintCollectors.sumPeriod((Period p)->p));}}}
}
