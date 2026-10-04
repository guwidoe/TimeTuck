// Original public Community aggregate benchmark; no upstream-method credit.
import java.io.*;
import java.util.*;
import java.util.function.Function;
import ai.timefold.solver.core.api.score.stream.ConstraintCollectors;
import ai.timefold.solver.core.api.score.stream.uni.*;
public class AggregateBenchmark {
 record Item(long key,long value){}
 static long[] numbers(String s){return Arrays.stream(s.split(",")).mapToLong(Long::parseLong).toArray();}
 static String encode(long[] a){var j=new StringJoiner(",");for(long v:a)j.add(""+v);return j.toString();}
 static long checksum(long[] a){long sum=0;for(int i=0;i<a.length;i++)sum+=(i+1)*a[i];return sum;}
 static void check(long[] a,long[] b){if(!Arrays.equals(a,b))throw new AssertionError("full observation mismatch");}
 static long[] set(SortedSet<Long> values){long[] out=new long[1+values.size()];out[0]=values.size();int i=1;for(long v:values)out[i++]=v;return out;}
 static long[] map(SortedMap<Long,Integer> values){long[] out=new long[1+2*values.size()];out[0]=values.size();int i=1;for(var e:values.entrySet()){out[i++]=e.getKey();out[i++]=e.getValue();}return out;}
 static <C,R> void run(String[] f,int iterations,int warmups,int samples,UniConstraintCollector<Item,C,R> collector,Function<R,long[]> observe){
  var items=new ArrayList<Item>();for(var row:f[2].split(";")){var v=numbers(row);items.add(new Item(v[0],v[1]));}var slots=numbers(f[3]);int selected=Integer.parseInt(f[4]),replacement=Integer.parseInt(f[5]),old=(int)slots[selected];var initial=numbers(f[6]);var expected=numbers(f[7]);
  for(int sample=0;sample<warmups+samples;sample++){
   var box=collector.supplier().get();var accumulator=(UniConstraintCollectorAccumulator<C,Item>)collector.accumulator();var handles=new ArrayList<UniConstraintCollectorValueHandle<Item>>(Collections.nCopies(slots.length,null));
   java.util.function.BiConsumer<Integer,Integer> put=(slot,item)->{var h=handles.get(slot);if(item<0){if(h!=null)h.remove();handles.set(slot,null);}else{if(h==null){h=accumulator.intoGroup(box);h.add(items.get(item));handles.set(slot,h);}else h.replaceWith(items.get(item));}};
   for(int i=0;i<slots.length;i++)put.accept(i,(int)slots[i]);check(observe.apply(collector.finisher().apply(box)),initial);
   long[] trial=null,restored=null;long total=0,start=System.nanoTime();
   for(int i=0;i<iterations;i++){put.accept(selected,replacement);trial=observe.apply(collector.finisher().apply(box));check(trial,expected);put.accept(selected,old);restored=observe.apply(collector.finisher().apply(box));check(restored,initial);total+=31*checksum(trial)+checksum(restored);}
   long elapsed=System.nanoTime()-start;put.accept(selected,replacement);var committed=observe.apply(collector.finisher().apply(box));check(committed,expected);
   if(sample>=warmups)System.out.println(f[0]+"\t"+elapsed+"\t"+total+"\t"+encode(trial)+"\t"+encode(restored)+"\t"+encode(committed));
  }
 }
 public static void main(String[] args)throws Exception{int iterations=Integer.parseInt(args[0]),warmups=Integer.parseInt(args[1]),samples=Integer.parseInt(args[2]);if(iterations<1||iterations>10000||warmups<0||samples<1)throw new IllegalArgumentException("counts");try(var in=new BufferedReader(new InputStreamReader(System.in))){String line;while((line=in.readLine())!=null){var f=line.split("\t",-1);switch(f[1]){
  case "sum"->run(f,iterations,warmups,samples,ConstraintCollectors.sum(Item::value),v->new long[]{v});
  case "set"->run(f,iterations,warmups,samples,ConstraintCollectors.toSortedSet(Item::value),AggregateBenchmark::set);
  case "map"->run(f,iterations,warmups,samples,ConstraintCollectors.toSortedMap(Item::key,i->(int)i.value(),Integer::sum),AggregateBenchmark::map);
  default->throw new IllegalArgumentException("mode");
 }}}}
}
