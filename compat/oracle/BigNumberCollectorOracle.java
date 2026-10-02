// Original adapter using only pinned Community public collector APIs.
import java.io.*;
import java.math.*;
import java.util.*;
import java.util.function.Function;
import ai.timefold.solver.core.api.score.stream.ConstraintCollectors;
import ai.timefold.solver.core.api.score.stream.uni.*;
public final class BigNumberCollectorOracle {
    static void emit(String id,int state,Object value) {
        String result=value==null?"-":value instanceof BigDecimal d?d.unscaledValue()+":"+d.scale():value.toString();
        System.out.println(id+"\t"+state+"\t"+result);
    }
    static <T,C,R> void run(String id,String ops,Function<String,T> parse,UniConstraintCollector<T,C,R> collector) {
        C container=collector.supplier().get();
        var accumulator=(UniConstraintCollectorAccumulator<C,T>)collector.accumulator();
        var slots=new HashMap<Integer,UniConstraintCollectorValueHandle<T>>();
        int state=0;emit(id,state,collector.finisher().apply(container));
        if(ops.isEmpty()) return;
        for(String op:ops.split(";",-1)) {
            var fields=op.split(":",3);int slot=Integer.parseInt(fields[1]);
            if(fields[0].equals("p") && fields.length==3) {
                T value=parse.apply(fields[2]);var handle=slots.get(slot);
                if(handle==null) {handle=accumulator.intoGroup(container);handle.add(value);slots.put(slot,handle);}
                else handle.replaceWith(value);
            } else if(fields[0].equals("r") && fields.length==2) {
                var handle=slots.remove(slot);if(handle!=null)handle.remove();
            } else throw new IllegalArgumentException("Invalid operation");
            emit(id,++state,collector.finisher().apply(container));
        }
    }
    static BigDecimal decimal(String value) {
        var fields=value.split(":",-1);
        if(fields.length!=2)throw new IllegalArgumentException("Expected coefficient:scale");
        return new BigDecimal(new BigInteger(fields[0]),Integer.parseInt(fields[1]));
    }
    public static void main(String[] args) throws Exception {
        try(var reader=new BufferedReader(new InputStreamReader(System.in))) {
            for(String line;(line=reader.readLine())!=null;) {
                var f=line.split("\t",-1);
                if(f.length!=3)throw new IllegalArgumentException("Expected id, kind, operations");
                switch(f[1]) {
                    case "integer_sum" -> run(f[0],f[2],BigInteger::new,ConstraintCollectors.sumBigInteger((BigInteger v)->v));
                    case "integer_average" -> run(f[0],f[2],BigInteger::new,ConstraintCollectors.averageBigInteger((BigInteger v)->v));
                    case "decimal_sum" -> run(f[0],f[2],BigNumberCollectorOracle::decimal,ConstraintCollectors.sumBigDecimal((BigDecimal v)->v));
                    case "decimal_average" -> run(f[0],f[2],BigNumberCollectorOracle::decimal,ConstraintCollectors.averageBigDecimal((BigDecimal v)->v));
                    default -> throw new IllegalArgumentException("Unknown kind");
                }
            }
        }
    }
}
