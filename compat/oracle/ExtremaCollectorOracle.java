// Original public Community key-projected extrema comparison; no method credit.
import java.io.*;
import java.util.*;
import ai.timefold.solver.core.api.score.stream.ConstraintCollectors;
import ai.timefold.solver.core.api.score.stream.uni.*;
public class ExtremaCollectorOracle {
 record Item(int id,long key,long value){}
 static void emit(String id,int state,Item v){System.out.println(id+"\t"+state+"\t"+(v==null?"-":v.id()+","+v.key()+","+v.value()));}
 static <C> void run(String[] f,UniConstraintCollector<Item,C,Item> collector){
  var box=collector.supplier().get();var acc=(UniConstraintCollectorAccumulator<C,Item>)collector.accumulator();var items=new ArrayList<Item>();
  for(var row:f[2].split(";")){var v=row.split(",");items.add(new Item(items.size(),Long.parseLong(v[0]),Long.parseLong(v[1])));}
  var slots=new HashMap<Integer,Integer>();var handles=new HashMap<Integer,UniConstraintCollectorValueHandle<Item>>();
  java.util.function.BiConsumer<Integer,Integer> put=(slot,item)->{if(item<0){var h=handles.remove(slot);if(h!=null)h.remove();slots.remove(slot);}else{var h=handles.get(slot);if(h==null){h=acc.intoGroup(box);h.add(items.get(item));handles.put(slot,h);}else h.replaceWith(items.get(item));slots.put(slot,item);}};
  int state=0;emit(f[0],state++,collector.finisher().apply(box));
  for(var op:f[3].split(";")){if(op.isEmpty())continue;var a=op.split(":");int slot=Integer.parseInt(a[1]),item=Integer.parseInt(a[2]),old=slots.getOrDefault(slot,-1);
   put.accept(slot,item);emit(f[0],state++,collector.finisher().apply(box));if(a[0].equals("t")){put.accept(slot,old);emit(f[0],state++,collector.finisher().apply(box));}
  }
 }
 public static void main(String[] args)throws Exception{try(var in=new BufferedReader(new InputStreamReader(System.in))){String line;while((line=in.readLine())!=null){var f=line.split("\t",-1);try{if(f[1].equals("min"))run(f,ConstraintCollectors.min((Item i)->i,Item::key));else run(f,ConstraintCollectors.max((Item i)->i,Item::key));}catch(RuntimeException e){System.out.println(f[0]+"\tERROR\t"+e.getClass().getName());}}}}
}
