// Original public Community collector comparison; no upstream method credit.
import java.io.*;
import java.util.*;
import ai.timefold.solver.core.api.score.stream.ConstraintCollectors;
import ai.timefold.solver.core.api.score.stream.uni.*;
import ai.timefold.solver.core.api.score.stream.common.LoadBalance;
public class LoadBalanceOracle {
 record Item(int key,long load,long initial){}
 static void emit(String id,int state,LoadBalance<Integer> c){
  var loads=new ArrayList<String>();
  new TreeMap<>(c.loads()).forEach((k,v)->loads.add(k+":"+v));
  var u=c.unfairness();
  System.out.println(id+"\t"+state+"\t"+(loads.isEmpty()?"-":String.join(";",loads))+"\t"+u.unscaledValue()+"\t"+u.scale());
 }
 static <C> void run(String[] f,UniConstraintCollector<Item,C,LoadBalance<Integer>> collector){
  var box=collector.supplier().get();var acc=(UniConstraintCollectorAccumulator<C,Item>)collector.accumulator();var items=new ArrayList<Item>();
  for(var row:f[1].split(";")){var v=row.split(",");items.add(new Item(Integer.parseInt(v[0]),Long.parseLong(v[1]),Long.parseLong(v[2])));}
  var slots=new HashMap<Integer,Integer>();var handles=new HashMap<Integer,UniConstraintCollectorValueHandle<Item>>();
  java.util.function.BiConsumer<Integer,Integer> put=(slot,item)->{if(item<0){var h=handles.remove(slot);if(h!=null)h.remove();slots.remove(slot);}else{var h=handles.get(slot);if(h==null){h=acc.intoGroup(box);h.add(items.get(item));handles.put(slot,h);}else h.replaceWith(items.get(item));slots.put(slot,item);}};
  int state=0;emit(f[0],state++,collector.finisher().apply(box));
  for(var op:f[2].split(";")){if(op.isEmpty())continue;var a=op.split(":");int slot=Integer.parseInt(a[1]),item=Integer.parseInt(a[2]),old=slots.getOrDefault(slot,-1);
   put.accept(slot,item);emit(f[0],state++,collector.finisher().apply(box));if(a[0].equals("t")){put.accept(slot,old);emit(f[0],state++,collector.finisher().apply(box));}
  }
 }
 public static void main(String[] args)throws Exception {var in=new BufferedReader(new InputStreamReader(System.in));String line;
  while((line=in.readLine())!=null){var f=line.split("\t",-1);try{run(f,ConstraintCollectors.loadBalance(Item::key,Item::load,Item::initial));}catch(RuntimeException e){System.out.println(f[0]+"\tERROR\t"+e.getClass().getName());}}
 }
}
