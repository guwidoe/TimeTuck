// Original public set/sorted-set/merged-map/checked-sum comparison.
import java.io.*;
import java.util.*;
import java.util.function.Function;
import java.util.stream.Collectors;
import ai.timefold.solver.core.api.score.stream.ConstraintCollectors;
import ai.timefold.solver.core.api.score.stream.uni.*;
public class SetSumCollectorOracle {
 record Item(long key,long value){}
 static <C,R> void run(String[] f,UniConstraintCollector<Item,C,R> collector,Function<R,String> format){
  var items=new ArrayList<Item>();for(var row:f[2].split(";")){var a=row.split(",");items.add(new Item(Long.parseLong(a[0]),Long.parseLong(a[1])));}
  var box=collector.supplier().get();var acc=(UniConstraintCollectorAccumulator<C,Item>)collector.accumulator();var handles=new HashMap<Integer,UniConstraintCollectorValueHandle<Item>>();var slots=new HashMap<Integer,Integer>();
  java.util.function.BiConsumer<Integer,Integer> put=(slot,item)->{if(item<0){var h=handles.remove(slot);if(h!=null)h.remove();slots.remove(slot);}else{var h=handles.get(slot);if(h==null){h=acc.intoGroup(box);h.add(items.get(item));handles.put(slot,h);}else h.replaceWith(items.get(item));slots.put(slot,item);}};
  int state=0;System.out.println(f[0]+"\t"+state+"\t"+format.apply(collector.finisher().apply(box)));
  for(var op:f[3].split(";")){if(op.isEmpty())continue;var v=op.split(":");int slot=Integer.parseInt(v[1]),item=Integer.parseInt(v[2]),old=slots.getOrDefault(slot,-1);for(int next:new int[]{item,old}){state++;try{put.accept(slot,next);System.out.println(f[0]+"\t"+state+"\t"+format.apply(collector.finisher().apply(box)));}catch(ArithmeticException e){System.out.println(f[0]+"\t"+state+"\tOVERFLOW");return;}if(!v[0].equals("t"))break;}}
 }
 static String set(Collection<Long> v){return v.isEmpty()?"-":v.stream().sorted().map(Object::toString).collect(Collectors.joining(","));}
 public static void main(String[] args)throws Exception{try(var in=new BufferedReader(new InputStreamReader(System.in))){String line;while((line=in.readLine())!=null){var f=line.split("\t",-1);switch(f[1]){
  case "set" -> run(f,ConstraintCollectors.toSet(Item::value),SetSumCollectorOracle::set);
  case "sorted" -> run(f,ConstraintCollectors.toSortedSet(Item::value),SetSumCollectorOracle::set);
  case "map" -> run(f,ConstraintCollectors.toSortedMap(Item::key,i->(int)i.value(),Integer::sum),m->m.isEmpty()?"-":m.entrySet().stream().map(e->e.getKey()+":"+e.getValue()).collect(Collectors.joining(",")));
  case "sum" -> run(f,ConstraintCollectors.sum(Item::value),Object::toString);
  default -> throw new IllegalArgumentException("unknown mode");
 }}}}
}
