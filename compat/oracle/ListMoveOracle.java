// Original development oracle. Only pinned Timefold Community public APIs.
import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.util.ArrayList;
import java.util.List;
import java.util.stream.Collectors;
import ai.timefold.solver.core.api.domain.entity.PlanningEntity;
import ai.timefold.solver.core.api.domain.solution.PlanningEntityCollectionProperty;
import ai.timefold.solver.core.api.domain.solution.PlanningScore;
import ai.timefold.solver.core.api.domain.solution.PlanningSolution;
import ai.timefold.solver.core.api.domain.valuerange.ValueRangeProvider;
import ai.timefold.solver.core.api.domain.variable.*;
import ai.timefold.solver.core.api.score.SimpleScore;
import ai.timefold.solver.core.api.solver.SolutionManager;
import ai.timefold.solver.core.preview.api.domain.metamodel.PlanningSolutionMetaModel;
import ai.timefold.solver.core.preview.api.domain.metamodel.ElementPosition;
import ai.timefold.solver.core.preview.api.move.Move;
import ai.timefold.solver.core.preview.api.move.builtin.Moves;
import ai.timefold.solver.core.preview.api.move.test.MoveTester;
import ai.timefold.solver.core.preview.api.neighborhood.stream.dataset.sample.Range;

public final class ListMoveOracle {
    @PlanningEntity
    public static class Entity {
        @PlanningListVariable(valueRangeProviderRefs="values", allowsUnassignedValues=true)
        public List<Value> values = new ArrayList<>();
        public int id;
        public Entity() {}
        Entity(int id) { this.id=id; }
    }
    @PlanningEntity
    public static class Value {
        public int id;
        @InverseRelationShadowVariable(sourceVariableName="values") public Entity owner;
        @IndexShadowVariable(sourceVariableName="values") public Integer index;
        @PreviousElementShadowVariable(sourceVariableName="values") public Value previous;
        @NextElementShadowVariable(sourceVariableName="values") public Value next;
        public Value() {}
        Value(int id) { this.id=id; }
    }
    @PlanningSolution
    public static class Problem {
        @ValueRangeProvider(id="values") @PlanningEntityCollectionProperty public List<Value> values;
        @PlanningEntityCollectionProperty public List<Entity> entities;
        @PlanningScore public SimpleScore score;
        public Problem() {}
    }
    static String value(Value v) { return v==null ? "-" : Integer.toString(v.id); }
    static String state(Problem p) {
        String lists=p.entities.stream().map(e -> e.values.isEmpty() ? "-" : e.values.stream().map(v -> Integer.toString(v.id)).collect(Collectors.joining(","))).collect(Collectors.joining(";"));
        String shadows=p.values.stream().map(v -> v.owner==null ? "-" : v.owner.id+":"+v.index+":"+value(v.previous)+":"+value(v.next)).collect(Collectors.joining(","));
        return lists+"/"+shadows;
    }
    public static void main(String[] args) throws Exception {
        var meta=PlanningSolutionMetaModel.of(Problem.class,Entity.class,Value.class);
        var variable=meta.genuineEntity(Entity.class).listVariable("values",Value.class);
        try(var input=new BufferedReader(new InputStreamReader(System.in))) {
            for(String line;(line=input.readLine())!=null;) {
                String[] f=line.split("\t",-1);
                if(f.length!=4) throw new IllegalArgumentException("Expected four fields");
                var p=new Problem(); p.values=new ArrayList<>(); p.entities=new ArrayList<>();
                for(int i=0;i<Integer.parseInt(f[1]);i++) p.values.add(new Value(i));
                String[] lists=f[2].split(";",-1);
                for(int i=0;i<lists.length;i++) {
                    var entity=new Entity(i);p.entities.add(entity);
                    if(!lists[i].equals("-")) for(String v:lists[i].split(",")) entity.values.add(p.values.get(Integer.parseInt(v)));
                }
                SolutionManager.updateShadowVariables(p);
                var moves=new ArrayList<Move<Problem>>();
                if(!f[3].isEmpty()) for(String op:f[3].split(";")) {
                    String[] parts=op.split(",");
                    int[] v=new int[parts.length-1];for(int i=1;i<parts.length;i++) v[i-1]=Integer.parseInt(parts[i]);
                    moves.add(switch(parts[0]) {
                        case "a" -> Moves.assign(variable,p.values.get(v[0]),p.entities.get(v[1]),v[2]);
                        case "c" -> Moves.change(variable,p.entities.get(v[0]),v[1],p.entities.get(v[2]),v[3]);
                        case "r" -> Moves.change(variable,new Range<>(p.entities.get(v[0]),v[1],v[2]),ElementPosition.of(p.entities.get(v[3]),v[4]),v[5]==1);
                        case "x" -> Moves.swap(variable,new Range<>(p.entities.get(v[0]),v[1],v[2]),new Range<>(p.entities.get(v[3]),v[4],v[5]),v[6]==1);
                        case "s" -> Moves.swap(variable,p.entities.get(v[0]),v[1],p.entities.get(v[2]),v[3]);
                        case "u" -> Moves.unassign(variable,new Range<>(p.entities.get(v[0]),v[1],v[2]));
                        default -> throw new IllegalArgumentException("Unknown move");
                    });
                }
                // No-move cases compare initial shadow enrichment only. The
                // public Java compose API rejects an empty move collection.
                if(moves.isEmpty()) {
                    String unchanged=state(p);
                    System.out.println(f[0]+"\t"+unchanged+"\t"+unchanged+"\t"+unchanged);
                    continue;
                }
                var movement=Moves.compose(moves);
                var context=MoveTester.build(meta).using(p);
                String[] temporary={null};
                context.executeTemporarily(movement,view -> temporary[0]=state(p));
                String restored=state(p);
                context.execute(movement);
                System.out.println(f[0]+"\t"+temporary[0]+"\t"+restored+"\t"+state(p));
            }
        }
    }
}
