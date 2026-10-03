// Original conditional-aggregation probe; pinned Community public APIs only.
import java.io.*;
import java.util.*;
import ai.timefold.solver.core.api.score.HardSoftScore;
import ai.timefold.solver.core.api.score.stream.*;
import ai.timefold.solver.core.api.solver.*;

public final class ConditionalCollectorOracle {
    public static class Provider implements ConstraintProvider {
        @Override public Constraint[] defineConstraints(ConstraintFactory factory) {
            var input = factory.forEach(StreamOracle.Job.class);
            var count = input.groupBy(ConstraintCollectors.conditionally(
                    (StreamOracle.Job j) -> j.weights[j.value] > 0, ConstraintCollectors.count()));
            var sum = input.groupBy(j -> j.value, ConstraintCollectors.conditionally(
                    (StreamOracle.Job j) -> j.weights[j.value] > 0,
                    ConstraintCollectors.sum((StreamOracle.Job j) -> j.weights[j.value])));
            return new Constraint[]{
                count.reward(HardSoftScore.ONE_HARD, n -> n).asConstraint("selected-count"),
                count.reward(HardSoftScore.ONE_SOFT).asConstraint("global-presence"),
                sum.impact(HardSoftScore.ONE_SOFT, (key, n) -> key * 100L + n).asConstraint("keyed-sum")
            };
        }
    }
    static void emit(String id, int state, TreeMap<Integer,StreamOracle.Job> rows,
            StreamOracle.Problem p, SolutionManager<StreamOracle.Problem,HardSoftScore> manager) {
        p.jobs = new ArrayList<>(rows.values());
        var score = manager.update(p, SolutionUpdatePolicy.UPDATE_SCORE_ONLY);
        System.out.println(id+"\t"+state+"\t"+score.hardScore()+"\t"+score.softScore());
    }
    public static void main(String[] args) throws Exception {
        var manager = SolutionManager.<StreamOracle.Problem,HardSoftScore>create(
            SolverFactory.create(StreamOracle.config().withConstraintProviderClass(Provider.class)));
        try (var input = new BufferedReader(new InputStreamReader(System.in))) {
            for (String line; (line=input.readLine())!=null;) {
                var fields=line.split("\t",-1);
                if(fields.length!=2) throw new IllegalArgumentException("case,operations required");
                var p=new StreamOracle.Problem();p.values=List.of(0,1,2,3);p.edges=List.of();
                var rows=new TreeMap<Integer,StreamOracle.Job>();int state=0;
                emit(fields[0],state++,rows,p,manager);
                if(fields[1].isEmpty())continue;
                for(var encoded:fields[1].split(";",-1)) {
                    var op=encoded.split(":",-1);
                    if(op.length!=3 || !(op[0].equals("c")||op[0].equals("t")))throw new IllegalArgumentException("operation");
                    int id=Integer.parseInt(op[1]);if(id<0||id>=32)throw new IllegalArgumentException("id");
                    var old=rows.get(id);
                    if(op[2].equals("-"))rows.remove(id);
                    else {
                        var v=op[2].split(",",-1);if(v.length!=2)throw new IllegalArgumentException("row");
                        int key=Integer.parseInt(v[0]);long weight=Long.parseLong(v[1]);
                        if(!p.values.contains(key)||weight < -100||weight > 100)throw new IllegalArgumentException("bounds");
                        long[] weights=new long[4];weights[key]=weight;
                        rows.put(id,new StreamOracle.Job(id,key,false,weights));
                    }
                    emit(fields[0],state++,rows,p,manager);
                    if(op[0].equals("t")) {
                        if(old==null)rows.remove(id);else rows.put(id,old);
                        emit(fields[0],state++,rows,p,manager);
                    }
                }
            }
        }
    }
}
