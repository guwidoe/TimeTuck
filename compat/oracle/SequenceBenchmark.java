// Original public Community collector benchmark; no upstream method credit.
import java.io.*;
import java.util.*;
import ai.timefold.solver.core.api.score.stream.ConstraintCollectors;
import ai.timefold.solver.core.api.score.stream.uni.*;
import ai.timefold.solver.core.api.score.stream.common.*;
public class SequenceBenchmark {
    record Item(int id,int index) {}
    static long[] numbers(String s) {return Arrays.stream(s.split(",")).mapToLong(Long::parseLong).toArray();}
    static String encode(long[] a) {var j=new StringJoiner(",");for(long v:a)j.add(""+v);return j.toString();}
    static long checksum(long[] a) {long sum=0;for(int i=0;i<a.length;i++)sum+=(i+1)*a[i];return sum;}
    static long[] observe(SequenceChain<Item,Integer> chain,int pool) {
        var sequences=chain.getConsecutiveSequences();var breaks=chain.getBreaks();
        long[] out=new long[2+3*pool+8*sequences.size()+5*breaks.size()];Arrays.fill(out,-1);
        out[0]=sequences.size();out[1]=breaks.size();int cursor=2+3*pool,q=0;
        var breakIterator=breaks.iterator(); Break<Item,Integer> previous=null;
        for(var s:sequences) {
            var next=breakIterator.hasNext()?breakIterator.next():null;
            if(s.getPreviousBreak()!=previous || s.getNextBreak()!=next)throw new AssertionError("neighbor pointers");
            previous=next;
            for(var item:s.getItems()) {
                int offset=2+3*item.id();if(out[offset]!=-1)throw new AssertionError("duplicate output identity");
                out[offset]=1;out[offset+1]=item.index();out[offset+2]=q;
            }
            out[cursor++]=s.getCount();out[cursor++]=s.getLength();out[cursor++]=s.getFirstItem().index();out[cursor++]=s.getLastItem().index();
            out[cursor++]=s.isFirst()?1:0;out[cursor++]=s.isLast()?1:0;
            // Ordinal pointers were validated against actual public break objects above.
            out[cursor++]=s.getPreviousBreak()==null?-1:q-1;out[cursor++]=s.getNextBreak()==null?-1:q;
            q++;
        }

        for(var b:breaks) {
            out[cursor++]=b.getLength();out[cursor++]=b.getPreviousSequenceEnd().index();out[cursor++]=b.getNextSequenceStart().index();out[cursor++]=b.isFirst()?1:0;out[cursor++]=b.isLast()?1:0;
        }
        return out;
    }
    static void check(long[] a,long[] b) {if(!Arrays.equals(a,b))throw new AssertionError("full observation mismatch");}
    static <C> void run(String[] f,int iterations,int warmups,int samples,UniConstraintCollector<Item,C,SequenceChain<Item,Integer>> collector) {
        var points=numbers(f[1]);var slots=numbers(f[2]);int selected=Integer.parseInt(f[3]),replacement=Integer.parseInt(f[4]);int old=(int)slots[selected];
        var initial=numbers(f[5]);var expected=numbers(f[6]);
        var items=new ArrayList<Item>();for(int i=0;i<points.length;i++)items.add(new Item(i,Math.toIntExact(points[i])));
        for(int sample=0;sample<warmups+samples;sample++) {
            var box=collector.supplier().get();var accumulator=(UniConstraintCollectorAccumulator<C,Item>)collector.accumulator();
            var handles=new HashMap<Integer,UniConstraintCollectorValueHandle<Item>>();
            java.util.function.BiConsumer<Integer,Integer> put=(slot,item)-> {
                if(item<0) {var h=handles.remove(slot);if(h!=null)h.remove();}
                else {var h=handles.get(slot);if(h==null){h=accumulator.intoGroup(box);h.add(items.get(item));handles.put(slot,h);}else h.replaceWith(items.get(item));}
            };
            for(int i=0;i<slots.length;i++)put.accept(i,(int)slots[i]);
            check(observe(collector.finisher().apply(box),points.length),initial);
            long[] trial=null,restored=null;long sum=0,start=System.nanoTime();
            for(int i=0;i<iterations;i++) {
                put.accept(selected,replacement);trial=observe(collector.finisher().apply(box),points.length);check(trial,expected);
                put.accept(selected,old);restored=observe(collector.finisher().apply(box),points.length);check(restored,initial);
                sum+=31*checksum(trial)+checksum(restored);
            }
            long elapsed=System.nanoTime()-start;
            put.accept(selected,replacement);var committed=observe(collector.finisher().apply(box),points.length);check(committed,expected);
            if(sample>=warmups)System.out.println(f[0]+"\t"+elapsed+"\t"+sum+"\t"+encode(trial)+"\t"+encode(restored)+"\t"+encode(committed));
        }
    }
    public static void main(String[] args)throws Exception {
        int iterations=Integer.parseInt(args[0]),warmups=Integer.parseInt(args[1]),samples=Integer.parseInt(args[2]);
        if(iterations<1||iterations>10000||warmups<0||samples<1)throw new IllegalArgumentException("counts");
        var in=new BufferedReader(new InputStreamReader(System.in));String line;
        while((line=in.readLine())!=null)run(line.split("\t",-1),iterations,warmups,samples,ConstraintCollectors.toConsecutiveSequences((Item i)->i.index()));
    }
}
