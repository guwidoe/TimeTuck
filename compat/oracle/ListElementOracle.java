// Domain and suppliers adapted from Community testdomain/shadow/list_element,
// Apache-2.0. See compat/provenance.json. Generated inputs are original.
import java.io.*;
import java.util.*;
import java.util.stream.Collectors;
import ai.timefold.solver.core.api.domain.entity.PlanningEntity;
import ai.timefold.solver.core.api.domain.solution.*;
import ai.timefold.solver.core.api.domain.valuerange.ValueRangeProvider;
import ai.timefold.solver.core.api.domain.variable.*;
import ai.timefold.solver.core.api.score.SimpleScore;
import ai.timefold.solver.core.api.score.stream.*;
import ai.timefold.solver.core.api.solver.*;
import ai.timefold.solver.core.config.solver.SolverConfig;
import ai.timefold.solver.core.preview.api.domain.metamodel.PlanningSolutionMetaModel;
import ai.timefold.solver.core.preview.api.move.Move;
import ai.timefold.solver.core.preview.api.move.builtin.Moves;
import ai.timefold.solver.core.preview.api.move.test.MoveTester;
import ai.timefold.solver.core.preview.api.neighborhood.stream.dataset.sample.Range;
public final class ListElementOracle {
    @PlanningEntity public static class Owner {
        int id;
        @PlanningListVariable(allowsUnassignedValues=true) public List<Value> values=new ArrayList<>();
        public int start;
        @ShadowVariable(supplierName="lastSupplier") public Integer last;
        public Owner(){}
        public List<Value> getValues(){return values;} public void setValues(List<Value> v){values=v;}
        public Integer getLast(){return last;} public void setLast(Integer v){last=v;}
        @ShadowSources("values[].end") public Integer lastSupplier(){return values.isEmpty()?Integer.valueOf(start):values.get(values.size()-1).end;}
    }
    @PlanningEntity public static class Value {
        int id; int duration;
        @InverseRelationShadowVariable(sourceVariableName="values") public Owner owner;
        @PreviousElementShadowVariable(sourceVariableName="values") public Value previous;
        @ShadowVariable(supplierName="startSupplier") public Integer start;
        @ShadowVariable(supplierName="endSupplier") public Integer end;
        public Value(){}
        public Owner getOwner(){return owner;} public void setOwner(Owner v){owner=v;}
        public Value getPrevious(){return previous;} public void setPrevious(Value v){previous=v;}
        public Integer getStart(){return start;} public void setStart(Integer v){start=v;}
        public Integer getEnd(){return end;} public void setEnd(Integer v){end=v;}
        @ShadowSources({"owner","previous","previous.end"}) public Integer startSupplier(){return owner==null?null:previous==null?Integer.valueOf(owner.start):previous.end;}
        @ShadowSources("start") public Integer endSupplier(){return start==null?null:start+duration;}
    }
    @PlanningSolution public static class Problem {
        @PlanningEntityCollectionProperty public List<Owner> owners=new ArrayList<>();
        @PlanningEntityCollectionProperty @ValueRangeProvider public List<Value> values=new ArrayList<>();
        @PlanningScore public SimpleScore score;
        public Problem(){}
    }
    public static class Provider implements ConstraintProvider {
        public Constraint[] defineConstraints(ConstraintFactory f){return new Constraint[]{
            f.forEachIncludingUnassigned(Value.class).filter(v->v.owner==null).penalize(SimpleScore.of(100)).asConstraint("Assign all values"),
            f.forEach(Owner.class).penalize(SimpleScore.ONE,o->o.last).asConstraint("Minimize last end time")};}
    }
    @PlanningEntity public static class MixedOwner {
        @PlanningListVariable public List<MixedValue> values=new ArrayList<>();
        @ShadowVariable(supplierName="totalSupplier") public Integer total;
        public MixedOwner(){}
        public List<MixedValue> getValues(){return values;} public void setValues(List<MixedValue> v){values=v;}
        public Integer getTotal(){return total;} public void setTotal(Integer v){total=v;}
        @ShadowSources("values[].padded") public Integer totalSupplier(){int sum=0;for(var v:values){if(v.padded==null)return null;sum+=v.padded;}return sum;}
    }
    @PlanningEntity public static class MixedValue {
        int id;
        @PlanningVariable(allowsUnassigned=true) public Integer duration;
        @ShadowVariable(supplierName="paddedSupplier") public Integer padded;
        public MixedValue(){}
        public Integer getDuration(){return duration;} public void setDuration(Integer v){duration=v;}
        public Integer getPadded(){return padded;} public void setPadded(Integer v){padded=v;}
        @ShadowSources("duration") public Integer paddedSupplier(){return duration==null?null:duration+1;}
    }
    @PlanningSolution public static class MixedProblem {
        @PlanningEntityCollectionProperty public List<MixedOwner> owners=new ArrayList<>();
        @PlanningEntityCollectionProperty @ValueRangeProvider public List<MixedValue> values=new ArrayList<>();
        @ValueRangeProvider public List<Integer> getDurations(){return List.of(1,2,3,4,5,6,7,8,9);}
        @PlanningScore public SimpleScore score;
        public MixedProblem(){}
    }
    static String text(Object v){return v==null?"-":v.toString();}
    static String row(List<?> vs){return vs.isEmpty()?"_":vs.stream().map(ListElementOracle::text).collect(Collectors.joining(","));}
    static List<Integer> parse(String s){if(s.equals("_"))return new ArrayList<>();return Arrays.stream(s.split(",",-1)).map(v->v.equals("-")?null:Integer.valueOf(v)).collect(Collectors.toList());}
    static int num(String s){return Integer.parseInt(s);}
    static String lists(List<List<Integer>> vs){return vs.stream().map(ListElementOracle::row).collect(Collectors.joining(";"));}
    static void emit(String id,int state,Problem p,SolutionManager<Problem,SimpleScore> manager){
        // Keep MoveTester shadow observations untouched. Rebuild a separate
        // solution from genuine/fixed inputs for a full public score update.
        // UPDATE_SCORE_ONLY returned zero for this declarative list fixture.
        var copy=new Problem();
        for(var v:p.values){var x=new Value();x.id=v.id;x.duration=v.duration;copy.values.add(x);}
        for(var o:p.owners){var x=new Owner();x.id=o.id;x.start=o.start;for(var v:o.values)x.values.add(copy.values.get(v.id));copy.owners.add(x);}
        var score=manager.update(copy,SolutionUpdatePolicy.UPDATE_ALL);
        System.out.println(String.join("\t",id,""+state,lists(p.owners.stream().map(o->o.values.stream().map(v->v.id).toList()).toList()),
            row(p.values.stream().map(v->v.duration).toList()),row(p.values.stream().map(v->v.owner==null?null:v.owner.id).toList()),
            row(p.values.stream().map(v->v.previous==null?null:v.previous.id).toList()),row(p.values.stream().map(v->v.start).toList()),
            row(p.values.stream().map(v->v.end).toList()),row(p.owners.stream().map(o->o.last).toList()),""+score.score()));
    }
    static void emitMixed(String id,int state,MixedProblem p){
        System.out.println(String.join("\t",id,""+state,lists(p.owners.stream().map(o->o.values.stream().map(v->v.id).toList()).toList()),
            row(p.values.stream().map(v->v.duration).toList()),"_","_","_",row(p.values.stream().map(v->v.padded).toList()),
            row(p.owners.stream().map(o->o.total).toList()),"0"));
    }
    static void ordered(String[] f){
        var p=new Problem();var durations=parse(f[3]);var starts=parse(f[2]);var initial=f[4].split(";",-1);
        for(int i=0;i<durations.size();i++){var v=new Value();v.id=i;v.duration=durations.get(i);p.values.add(v);}
        for(int i=0;i<starts.size();i++){var o=new Owner();o.id=i;o.start=starts.get(i);for(var v:parse(initial[i]))o.values.add(p.values.get(v));p.owners.add(o);}
        var meta=PlanningSolutionMetaModel.of(Problem.class,Owner.class,Value.class);
        var list=meta.genuineEntity(Owner.class).listVariable("values",Value.class);
        var manager=SolutionManager.<Problem,SimpleScore>create(SolverFactory.create(new SolverConfig().withSolutionClass(Problem.class).withEntityClasses(Owner.class,Value.class).withConstraintProviderClass(Provider.class)));
        var context=MoveTester.build(meta).using(p);int state=0;emit(f[0],state++,p,manager);
        for(var batch:f[5].split("\\|")){if(batch.isEmpty())continue;var parts=batch.split("@",-1);var moves=new ArrayList<Move<Problem>>();
            if(!parts[1].isEmpty())for(var op:parts[1].split(";")){var v=op.split(",");moves.add(switch(v[0]){
                case "a"->Moves.assign(list,p.values.get(num(v[1])),p.owners.get(num(v[2])),num(v[3]));
                case "c"->Moves.change(list,p.owners.get(num(v[1])),num(v[2]),p.owners.get(num(v[3])),num(v[4]));
                case "s"->Moves.swap(list,p.owners.get(num(v[1])),num(v[2]),p.owners.get(num(v[3])),num(v[4]));
                case "u"->Moves.unassign(list,new Range<>(p.owners.get(num(v[1])),num(v[2]),num(v[3])));
                default->throw new IllegalArgumentException("ordered move");});}
            if(moves.isEmpty()){emit(f[0],state++,p,manager);if(parts[0].equals("t"))emit(f[0],state++,p,manager);continue;}
            var move=Moves.compose(moves);
            if(parts[0].equals("t")){final int n=state++;context.executeTemporarily(move,view->emit(f[0],n,p,manager));emit(f[0],state++,p,manager);}
            else if(parts[0].equals("c")){context.execute(move);emit(f[0],state++,p,manager);}else throw new IllegalArgumentException("mode");
        }
    }
    static void mixed(String[] f){
        var p=new MixedProblem();var durations=parse(f[3]);var initial=f[4].split(";",-1);
        for(int i=0;i<durations.size();i++){var v=new MixedValue();v.id=i;v.duration=durations.get(i);p.values.add(v);}
        for(var row:initial){var o=new MixedOwner();for(var v:parse(row))o.values.add(p.values.get(v));p.owners.add(o);}
        var meta=PlanningSolutionMetaModel.of(MixedProblem.class,MixedOwner.class,MixedValue.class);
        var list=meta.genuineEntity(MixedOwner.class).listVariable("values",MixedValue.class);
        var basic=meta.genuineEntity(MixedValue.class).basicVariable("duration",Integer.class);
        var context=MoveTester.build(meta).using(p);int state=0;emitMixed(f[0],state++,p);
        for(var batch:f[5].split("\\|")){if(batch.isEmpty())continue;var parts=batch.split("@",-1);var moves=new ArrayList<Move<MixedProblem>>();
            if(!parts[1].isEmpty())for(var op:parts[1].split(";")){var v=op.split(",");moves.add(switch(v[0]){
                case "b"->Moves.change(basic,p.values.get(num(v[1])),v[2].equals("-")?null:Integer.valueOf(v[2]));
                case "a"->Moves.assign(list,p.values.get(num(v[1])),p.owners.get(num(v[2])),num(v[3]));
                case "c"->Moves.change(list,p.owners.get(num(v[1])),num(v[2]),p.owners.get(num(v[3])),num(v[4]));
                case "s"->Moves.swap(list,p.owners.get(num(v[1])),num(v[2]),p.owners.get(num(v[3])),num(v[4]));
                default->throw new IllegalArgumentException("mixed move");});}
            if(moves.isEmpty()){emitMixed(f[0],state++,p);if(parts[0].equals("t"))emitMixed(f[0],state++,p);continue;}
            var move=Moves.compose(moves);
            if(parts[0].equals("t")){final int n=state++;context.executeTemporarily(move,view->emitMixed(f[0],n,p));emitMixed(f[0],state++,p);}
            else if(parts[0].equals("c")){context.execute(move);emitMixed(f[0],state++,p);}else throw new IllegalArgumentException("mode");
        }
    }
    public static void main(String[] args)throws Exception{
        try(var input=new BufferedReader(new InputStreamReader(System.in))){for(String line;(line=input.readLine())!=null;){
            var f=line.split("\t",-1);if(f.length!=6)throw new IllegalArgumentException("six fields");
            if(f[1].equals("o"))ordered(f);else if(f[1].equals("m"))mixed(f);else throw new IllegalArgumentException("model");
        }}
    }
}
