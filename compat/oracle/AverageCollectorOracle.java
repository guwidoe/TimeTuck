// Original development oracle using only pinned Community public collector APIs.
import java.io.*;
import java.util.*;
import ai.timefold.solver.core.api.score.stream.ConstraintCollectors;
import ai.timefold.solver.core.api.score.stream.uni.*;

public final class AverageCollectorOracle {
    static <C> void run(String id, String operations, UniConstraintCollector<Long,C,Double> collector) {
        C container=collector.supplier().get();
        var accumulator=(UniConstraintCollectorAccumulator<C,Long>)collector.accumulator();
        var slots=new HashMap<Integer,UniConstraintCollectorValueHandle<Long>>();
        int state=0;
        emit(id,state++,collector.finisher().apply(container));
        if (operations.isEmpty()) return;
        for (String operation:operations.split(";",-1)) {
          try {
            String[] fields=operation.split(":",-1);
            int slot=Integer.parseInt(fields[1]);
            if (fields[0].equals("p") && fields.length==3) {
                long value=Long.parseLong(fields[2]);
                var handle=slots.get(slot);
                if (handle==null) {
                    handle=accumulator.intoGroup(container);
                    handle.add(value);
                    slots.put(slot,handle);
                } else handle.replaceWith(value);
            } else if (fields[0].equals("r") && fields.length==2) {
                var handle=slots.remove(slot);
                if (handle!=null) handle.remove();
            } else throw new IllegalArgumentException("Invalid average operation");
            emit(id,state++,collector.finisher().apply(container));
          } catch (ArithmeticException error) {
            System.out.println(id+"\t"+state+"\tOVERFLOW");
            return; // Do not reuse a Java container after a rejected update.
          }
        }
    }
    static void emit(String id,int state,Double value) {
        System.out.println(id+"\t"+state+"\t"+(value==null?"-":String.format(Locale.ROOT,"%016x",Double.doubleToRawLongBits(value))));
    }
    public static void main(String[] args) throws Exception {
        try (var reader=new BufferedReader(new InputStreamReader(System.in))) {
            for (String line;(line=reader.readLine())!=null;) {
                String[] fields=line.split("\t",-1);
                if (fields.length!=2) throw new IllegalArgumentException("Expected case and operations");
                run(fields[0],fields[1],ConstraintCollectors.average((Long value)->value));
            }
        }
    }
}
