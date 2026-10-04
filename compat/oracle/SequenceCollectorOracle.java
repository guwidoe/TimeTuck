// Original public Community API driver. No upstream method credit.
import java.io.*;
import java.util.*;
import ai.timefold.solver.core.api.score.stream.ConstraintCollectors;
import ai.timefold.solver.core.api.score.stream.uni.*;
import ai.timefold.solver.core.api.score.stream.common.*;
public class SequenceCollectorOracle {
    record Item(int id, int index) {}
    static String flag(boolean b) { return b ? "1" : "0"; }
    static void emit(String id, int state, SequenceChain<Item,Integer> chain) {
        var ss = new ArrayList<String>(); var bs = new ArrayList<String>();
        var seqs = new ArrayList<>(chain.getConsecutiveSequences());
        var breaks = new ArrayList<>(chain.getBreaks());
        if (chain.getFirstSequence() != (seqs.isEmpty() ? null : seqs.getFirst()) ||
            chain.getLastSequence() != (seqs.isEmpty() ? null : seqs.getLast()) ||
            chain.getFirstBreak() != (breaks.isEmpty() ? null : breaks.getFirst()) ||
            chain.getLastBreak() != (breaks.isEmpty() ? null : breaks.getLast())) throw new AssertionError("chain endpoints");
        for (int i=0;i<seqs.size();i++) {
            var s=seqs.get(i);
            if(s.getPreviousBreak() != (i==0 ? null : breaks.get(i-1)) ||
               s.getNextBreak() != (i+1==seqs.size() ? null : breaks.get(i))) throw new AssertionError("neighbor breaks");
            var ids = new ArrayList<String>();
            for (var x : s.getItems()) ids.add(""+x.id());
            ss.add(s.getCount()+","+s.getLength()+","+flag(s.isFirst())+","+flag(s.isLast())+","+
                (s.getPreviousBreak()==null ? "-" : s.getPreviousBreak().getLength())+","+
                (s.getNextBreak()==null ? "-" : s.getNextBreak().getLength())+","+
                s.getFirstItem().id()+","+s.getLastItem().id()+":"+String.join(",",ids));
        }
        for (var b : breaks) bs.add(b.getLength()+","+flag(b.isFirst())+","+flag(b.isLast())+","+
            b.getPreviousSequenceEnd().id()+","+b.getNextSequenceStart().id());
        System.out.println(id+"\t"+state+"\t"+(ss.isEmpty()?"-":String.join(";",ss))+"\t"+(bs.isEmpty()?"-":String.join(";",bs)));
    }
    static <C> void run(String[] f, UniConstraintCollector<Item,C,SequenceChain<Item,Integer>> collector) {
        var box=collector.supplier().get();
        var accumulator=(UniConstraintCollectorAccumulator<C,Item>)collector.accumulator();
        var points=f[1].split(","); var items=new ArrayList<Item>();
        for(int i=0;i<points.length;i++)items.add(new Item(i,Integer.parseInt(points[i])));
        var handles=new HashMap<Integer,UniConstraintCollectorValueHandle<Item>>();
        var slots=new HashMap<Integer,Integer>();
        java.util.function.BiConsumer<Integer,Integer> put=(slot,item)-> {
            if(item<0) {var h=handles.remove(slot);if(h!=null)h.remove(); slots.remove(slot);}
            else {var h=handles.get(slot); if(h==null){h=accumulator.intoGroup(box);h.add(items.get(item));handles.put(slot,h);}
                else h.replaceWith(items.get(item));slots.put(slot,item);}
        };
        int state=0;emit(f[0],state++,collector.finisher().apply(box));
        if(f.length>2&&!f[2].isEmpty())for(var op:f[2].split(";")) {
            var a=op.split(":");int slot=Integer.parseInt(a[1]), item=Integer.parseInt(a[2]);
            int old=slots.getOrDefault(slot,-1); put.accept(slot,item);
            emit(f[0],state++,collector.finisher().apply(box));
            if(a[0].equals("t")){put.accept(slot,old);emit(f[0],state++,collector.finisher().apply(box));}
        }
    }
    public static void main(String[] args)throws Exception {
        var in=new BufferedReader(new InputStreamReader(System.in)); String line;
        while((line=in.readLine())!=null)run(line.split("\t",-1),ConstraintCollectors.toConsecutiveSequences((Item x)->x.index()));
    }
}
