// Original development adapter; pinned Community public APIs only.
import java.io.*;
import java.util.*;
import java.util.stream.Collectors;
import ai.timefold.solver.core.api.domain.entity.*;
import ai.timefold.solver.core.api.domain.solution.*;
import ai.timefold.solver.core.api.domain.valuerange.ValueRangeProvider;
import ai.timefold.solver.core.api.domain.variable.*;
import ai.timefold.solver.core.api.score.HardSoftScore;
import ai.timefold.solver.core.api.score.stream.*;
import ai.timefold.solver.core.api.solver.*;
import ai.timefold.solver.core.config.solver.SolverConfig;
import ai.timefold.solver.core.preview.api.domain.metamodel.PlanningSolutionMetaModel;
import ai.timefold.solver.core.preview.api.move.Move;
import ai.timefold.solver.core.preview.api.move.builtin.Moves;
import ai.timefold.solver.core.preview.api.move.test.MoveTester;
import ai.timefold.solver.core.preview.api.neighborhood.stream.dataset.sample.Range;
import static ai.timefold.solver.core.api.score.stream.Joiners.equal;

public final class MixedStreamOracle {
    @PlanningEntity public static class Slot {
        public int id;
        @PlanningVariable(valueRangeProviderRefs="coefficients") public Integer coefficient;
        @PlanningPin public boolean pinned;
        public Slot() {}
    }
    @PlanningEntity public static class Owner {
        public int id;
        @PlanningListVariable(valueRangeProviderRefs="values",allowsUnassignedValues=true)
        public List<Value> values=new ArrayList<>();
        public Owner() {}
    }
    @PlanningEntity public static class Value {
        public int id;
        @InverseRelationShadowVariable(sourceVariableName="values") public Owner owner;
        @IndexShadowVariable(sourceVariableName="values") public Integer index;
        @PreviousElementShadowVariable(sourceVariableName="values") public Value previous;
        @NextElementShadowVariable(sourceVariableName="values") public Value next;
        public Value() {}
    }
    @PlanningSolution public static class Problem {
        @ValueRangeProvider(id="coefficients") @ProblemFactCollectionProperty public List<Integer> coefficients=List.of(-2,0,3);
        @PlanningEntityCollectionProperty public List<Slot> slots=new ArrayList<>();
        @PlanningEntityCollectionProperty public List<Owner> owners=new ArrayList<>();
        @ValueRangeProvider(id="values") @PlanningEntityCollectionProperty public List<Value> values=new ArrayList<>();
        @PlanningScore public HardSoftScore score;
        public Problem() {}
    }
    public static class Provider implements ConstraintProvider {
        public Constraint[] defineConstraints(ConstraintFactory f) {
            var slots=f.forEachIncludingUnassigned(Slot.class);
            return new Constraint[] {
                f.forEach(Owner.class).penalize(HardSoftScore.ONE_HARD,o->Math.max(0,o.values.size()-2)).asConstraint("capacity"),
                f.forEach(Owner.class).join(slots,equal(o->o.id,s->s.id))
                    .impact(HardSoftScore.ONE_SOFT,(o,s)->(long)(s.coefficient==null?0:s.coefficient)*o.values.size()).asConstraint("loads"),
                f.forEachIncludingUnassigned(Value.class).filter(v->v.owner!=null).join(slots,equal(v->v.owner.id,s->s.id))
                    .impact(HardSoftScore.ONE_SOFT,(v,s)->(long)(s.coefficient==null?0:s.coefficient)*v.id+v.index
                        +(v.previous==null?0:v.previous.id)-(v.next==null?0:v.next.id)).asConstraint("shadows")
            };
        }
    }
    static String id(Value v){return v==null?"-":Integer.toString(v.id);}
    static String shadow(Value v){
        if(v.owner==null){
            if(v.index!=null||v.previous!=null||v.next!=null)throw new IllegalStateException("unassigned value retains shadows");
            return "-";
        }
        return v.owner.id+":"+v.index+":"+id(v.previous)+":"+id(v.next);
    }
    static Value value(Problem p,int id){return p.values.stream().filter(v->v.id==id).findFirst().orElseThrow();}
    static void emit(String id,int state,Problem p,SolutionManager<Problem,HardSoftScore> manager){
        var score=manager.update(p,SolutionUpdatePolicy.UPDATE_SCORE_ONLY);
        String basic=p.slots.stream().map(s->s.coefficient==null?"-":s.coefficient.toString()).collect(Collectors.joining(","));
        String lists=p.owners.stream().map(o->o.values.isEmpty()?"-":o.values.stream().map(MixedStreamOracle::id).collect(Collectors.joining(","))).collect(Collectors.joining(";"));
        String shadows=p.values.stream().map(MixedStreamOracle::shadow).collect(Collectors.joining(","));
        boolean complete=p.slots.stream().allMatch(s->s.coefficient!=null);
        System.out.println(id+"\t"+state+"\t"+(complete?1:0)+"\t"+score.hardScore()+"\t"+score.softScore()+"\t"+basic+"\t"+lists+"\t"+shadows);
    }
    public static void main(String[] args)throws Exception {
        var meta=PlanningSolutionMetaModel.of(Problem.class,Slot.class,Owner.class,Value.class);
        var basic=meta.genuineEntity(Slot.class).basicVariable("coefficient",Integer.class);
        var lists=meta.genuineEntity(Owner.class).listVariable("values",Value.class);
        var config=new SolverConfig().withSolutionClass(Problem.class).withEntityClasses(Slot.class,Owner.class,Value.class).withConstraintProviderClass(Provider.class);
        var manager=SolutionManager.<Problem,HardSoftScore>create(SolverFactory.create(config));
        try(var input=new BufferedReader(new InputStreamReader(System.in))){
            for(String line;(line=input.readLine())!=null;){
                var f=line.split("\t",-1);if(f.length!=6)throw new IllegalArgumentException("six fields required");
                var p=new Problem();var coefficients=f[1].split(",",-1);var pins=f[2].split(",",-1);var initial=f[4].split(";",-1);
                if(coefficients.length!=pins.length||coefficients.length!=initial.length)throw new IllegalArgumentException("shape mismatch");
                if(!f[3].equals("-"))for(var bit:f[3].split(",")){var v=new Value();v.id=Integer.parseInt(bit);p.values.add(v);}
                for(int i=0;i<coefficients.length;i++){
                    var s=new Slot();s.id=i;s.coefficient=coefficients[i].equals("-")?null:Integer.valueOf(coefficients[i]);s.pinned=pins[i].equals("1");p.slots.add(s);
                    var o=new Owner();o.id=i;if(!initial[i].equals("-"))for(var bit:initial[i].split(","))o.values.add(value(p,Integer.parseInt(bit)));p.owners.add(o);
                }
                SolutionManager.updateShadowVariables(p);int state=0;emit(f[0],state++,p,manager);
                var context=MoveTester.build(meta).using(p);
                if(f[5].isEmpty())continue;
                for(var batch:f[5].split("\\|")){
                    var parts=batch.split("@",-1);if(parts.length!=2||!(parts[0].equals("t")||parts[0].equals("c")))throw new IllegalArgumentException("batch mode");
                    var moves=new ArrayList<Move<Problem>>();
                    if(!parts[1].isEmpty())for(var op:parts[1].split(";")){
                        var v=op.split(",");
                        moves.add(switch(v[0]){
                            case "b"->Moves.change(basic,p.slots.get(Integer.parseInt(v[1])),v[2].equals("-")?null:Integer.valueOf(v[2]));
                            case "a"->Moves.assign(lists,value(p,Integer.parseInt(v[1])),p.owners.get(Integer.parseInt(v[2])),Integer.parseInt(v[3]));
                            case "c"->Moves.change(lists,p.owners.get(Integer.parseInt(v[1])),Integer.parseInt(v[2]),p.owners.get(Integer.parseInt(v[3])),Integer.parseInt(v[4]));
                            case "s"->Moves.swap(lists,p.owners.get(Integer.parseInt(v[1])),Integer.parseInt(v[2]),p.owners.get(Integer.parseInt(v[3])),Integer.parseInt(v[4]));
                            case "u"->Moves.unassign(lists,new Range<>(p.owners.get(Integer.parseInt(v[1])),Integer.parseInt(v[2]),Integer.parseInt(v[3])));
                            default->throw new IllegalArgumentException("move kind");
                        });
                    }
                    if(moves.isEmpty()){emit(f[0],state++,p,manager);if(parts[0].equals("t"))emit(f[0],state++,p,manager);continue;}
                    var movement=Moves.compose(moves);
                    if(parts[0].equals("t")){final int observation=state++;context.executeTemporarily(movement,view->emit(f[0],observation,p,manager));emit(f[0],state++,p,manager);}
                    else{context.execute(movement);emit(f[0],state++,p,manager);}
                }
            }
        }
    }
}
