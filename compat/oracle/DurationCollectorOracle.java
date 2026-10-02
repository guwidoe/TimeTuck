// Original probe using only pinned Community public collector APIs.
import java.io.*;
import java.math.BigInteger;
import java.time.Duration;
import java.util.*;
import ai.timefold.solver.core.api.score.stream.ConstraintCollectors;
import ai.timefold.solver.core.api.score.stream.uni.*;

public final class DurationCollectorOracle {
    static final BigInteger BILLION=BigInteger.valueOf(1000000000);
    static Duration duration(String nanos) {
        var parts=new BigInteger(nanos).divideAndRemainder(BILLION);
        return Duration.ofSeconds(parts[0].longValueExact(),parts[1].longValueExact());
    }
    static void emit(String id,int state,Duration value) {
        String result=value==null?"-":BigInteger.valueOf(value.getSeconds()).multiply(BILLION).add(BigInteger.valueOf(value.getNano())).toString();
        System.out.println(id+"\t"+state+"\t"+result);
    }
    static <C> void run(String id,String ops,UniConstraintCollector<Duration,C,Duration> collector) {
        C container=collector.supplier().get();
        var accumulator=(UniConstraintCollectorAccumulator<C,Duration>)collector.accumulator();
        var slots=new HashMap<Integer,UniConstraintCollectorValueHandle<Duration>>();
        int state=0;
        emit(id,state,collector.finisher().apply(container));
        if(ops.isEmpty()) return;
        for(String op:ops.split(";",-1)) {
            state++;
            String phase="update";
            try {
                var fields=op.split(":",-1);
                int slot=Integer.parseInt(fields[1]);
                if(fields[0].equals("p") && fields.length==3) {
                    var value=duration(fields[2]);
                    var handle=slots.get(slot);
                    if(handle==null) {
                        handle=accumulator.intoGroup(container);
                        handle.add(value);
                        slots.put(slot,handle);
                    } else handle.replaceWith(value);
                } else if(fields[0].equals("r") && fields.length==2) {
                    var handle=slots.remove(slot);
                    if(handle!=null) handle.remove();
                } else throw new IllegalArgumentException("Invalid duration operation");
                phase="finish";
                emit(id,state,collector.finisher().apply(container));
            } catch(ArithmeticException error) {
                System.out.println(id+"\t"+state+"\tOVERFLOW:"+phase);
                return; // No post-error Java container state is inspected.
            }
        }
    }
    public static void main(String[] args) throws Exception {
        try(var reader=new BufferedReader(new InputStreamReader(System.in))) {
            for(String line;(line=reader.readLine())!=null;) {
                var fields=line.split("\t",-1);
                if(fields.length!=3) throw new IllegalArgumentException("Expected id, kind and operations");
                switch(fields[1]) {
                    case "sum" -> run(fields[0],fields[2],ConstraintCollectors.sumDuration((Duration value)->value));
                    case "average" -> run(fields[0],fields[2],ConstraintCollectors.averageDuration((Duration value)->value));
                    default -> throw new IllegalArgumentException("Unknown kind");
                }
            }
        }
    }
}
