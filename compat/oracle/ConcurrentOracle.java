// Domain/supplier/scoring fixtures adapted from Community testdomain/shadow/concurrent,
// Apache-2.0. See compat/provenance.json. Generated inputs are original.
import java.io.*;
import java.util.*;
import java.util.stream.Collectors;
import ai.timefold.solver.core.api.domain.entity.PlanningEntity;
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
public class ConcurrentOracle {
    @PlanningEntity public static class Owner {
        int id;
        @PlanningListVariable public List<Value> values=new ArrayList<>();
        public Owner(){}
        public List<Value> getValues(){return values;} public void setValues(List<Value> v){values=v;}
    }
    @PlanningEntity public static class Value {
        int id;
        @InverseRelationShadowVariable(sourceVariableName="values") public Owner owner;
        @PreviousElementShadowVariable(sourceVariableName="values") public Value previous;
        @NextElementShadowVariable(sourceVariableName="values") public Value next;
        @IndexShadowVariable(sourceVariableName="values") public Integer index;
        @CascadingUpdateShadowVariable(targetMethodName="cascade") public Long cascading;
        public List<Value> group;
        @ShadowVariablesInconsistent public boolean inconsistent;
        @ShadowVariable(supplierName="readySupplier") public Long ready;
        @ShadowVariable(supplierName="startSupplier") public Long start;
        @ShadowVariable(supplierName="finishSupplier") public Long finish;
        public Value(){}
        public Owner getOwner(){return owner;} public void setOwner(Owner v){owner=v;}
        public Value getPrevious(){return previous;} public void setPrevious(Value v){previous=v;}
        public Value getNext(){return next;} public void setNext(Value v){next=v;}
        public Integer getIndex(){return index;} public void setIndex(Integer v){index=v;}
        public Long getCascading(){return cascading;} public void setCascading(Long v){cascading=v;}
        public List<Value> getGroup(){return group;} public void setGroup(List<Value> v){group=v;}
        public boolean isInconsistent(){return inconsistent;} public void setInconsistent(boolean v){inconsistent=v;}
        public Long getReady(){return ready;} public void setReady(Long v){ready=v;}
        public Long getStart(){return start;} public void setStart(Long v){start=v;}
        public Long getFinish(){return finish;} public void setFinish(Long v){finish=v;}
        @ShadowSources({"previous.finish","owner"}) public Long readySupplier(){if(previous!=null)return previous.finish+30;return owner==null?null:0L;}
        @ShadowSources(value={"ready","group[].ready"},alignmentKey="group") public Long startSupplier(){
            if(inconsistent)throw new IllegalStateException("inconsistent supplier");
            if(ready==null)return null;Long result=ready;
            if(group!=null)for(var v:group){if(v.inconsistent)throw new IllegalStateException("inconsistent member");if(v.ready!=null&&v.ready>result)result=v.ready;}
            return result;
        }
        @ShadowSources("start") public Long finishSupplier(){return start==null?null:start+30;}
        public void cascade(){cascading=index==null?null:1440L*index;}
    }
    @PlanningSolution public static class Problem {
        @PlanningEntityCollectionProperty public List<Owner> owners=new ArrayList<>();
        @PlanningEntityCollectionProperty @ValueRangeProvider public List<Value> values=new ArrayList<>();
        @PlanningScore public HardSoftScore score;
        public Problem(){}
    }
    public static class Provider implements ConstraintProvider {
        public Constraint[] defineConstraints(ConstraintFactory f){return new Constraint[]{
            f.forEachUnfiltered(Value.class).filter(v->v.inconsistent).penalize(HardSoftScore.ONE_HARD).asConstraint("Invalid visit"),
            f.forEach(Value.class).filter(v->v.owner!=null).penalize(HardSoftScore.ONE_SOFT,v->v.finish).asConstraint("Minimize finish time")};}
    }
    static String text(Object v){return v==null?"-":v.toString();}
    static String row(List<?> vs){return vs.isEmpty()?"_":vs.stream().map(ConcurrentOracle::text).collect(Collectors.joining(","));}
    static List<Integer> parse(String s){return s.equals("_")?List.of():Arrays.stream(s.split(",")).map(Integer::valueOf).toList();}
    static String lists(Problem p){return p.owners.stream().map(o->row(o.values.stream().map(v->v.id).toList())).collect(Collectors.joining(";"));}
    static Problem create(int n,String groups,String lists){
        var p=new Problem();for(int i=0;i<n;i++){var v=new Value();v.id=i;p.values.add(v);}
        if(!groups.equals("_"))for(var g:groups.split(";")){var members=parse(g).stream().map(p.values::get).toList();for(var v:members)v.group=members;}
        for(var l:lists.split(";")){var o=new Owner();o.id=p.owners.size();for(var v:parse(l))o.values.add(p.values.get(v));p.owners.add(o);}
        return p;
    }
    static void emit(String id,int state,Problem p,String groups,SolutionManager<Problem,HardSoftScore> manager){
        // Score a fresh solution; do not repair the observed MoveTester shadows.
        var copy=create(p.values.size(),groups,lists(p));var score=manager.update(copy,SolutionUpdatePolicy.UPDATE_ALL);
        var fields=new ArrayList<String>(List.of(id,""+state,lists(p),""+score.hardScore(),""+score.softScore()));
        fields.add(row(p.values.stream().map(v->v.owner==null?null:v.owner.id).toList()));
        fields.add(row(p.values.stream().map(v->v.index).toList()));
        fields.add(row(p.values.stream().map(v->v.previous==null?null:v.previous.id).toList()));
        fields.add(row(p.values.stream().map(v->v.next==null?null:v.next.id).toList()));
        fields.add(row(p.values.stream().map(v->v.cascading).toList()));
        fields.add(row(p.values.stream().map(v->v.ready).toList()));
        fields.add(row(p.values.stream().map(v->v.start).toList()));
        fields.add(row(p.values.stream().map(v->v.finish).toList()));
        fields.add(row(p.values.stream().map(v->v.inconsistent?1:0).toList()));
        System.out.println(String.join("\t",fields));
    }
    static int num(String v){return Integer.parseInt(v);}
    public static void main(String[] args)throws Exception {
        var meta=PlanningSolutionMetaModel.of(Problem.class,Owner.class,Value.class);
        var list=meta.genuineEntity(Owner.class).listVariable("values",Value.class);
        var manager=SolutionManager.<Problem,HardSoftScore>create(SolverFactory.create(new SolverConfig().withSolutionClass(Problem.class).withEntityClasses(Owner.class,Value.class).withConstraintProviderClass(Provider.class)));
        try(var input=new BufferedReader(new InputStreamReader(System.in))){for(String line;(line=input.readLine())!=null;){
            var f=line.split("\t",-1);if(f.length!=5)throw new IllegalArgumentException("five fields");
            try {
            var p=create(num(f[1]),f[2],f[3]);var context=MoveTester.build(meta).using(p);int state=0;emit(f[0],state++,p,f[2],manager);
            for(var batch:f[4].split("\\|")){if(batch.isEmpty())continue;var parts=batch.split("@",-1);if(parts.length!=2||!(parts[0].equals("c")||parts[0].equals("t")))throw new IllegalArgumentException("mode");
                var moves=new ArrayList<Move<Problem>>();
                if(!parts[1].isEmpty())for(var op:parts[1].split(";")){var v=op.split(",");moves.add(switch(v[0]){
                    case "a"->Moves.assign(list,p.values.get(num(v[1])),p.owners.get(num(v[2])),num(v[3]));
                    case "c"->Moves.change(list,p.owners.get(num(v[1])),num(v[2]),p.owners.get(num(v[3])),num(v[4]));
                    case "s"->Moves.swap(list,p.owners.get(num(v[1])),num(v[2]),p.owners.get(num(v[3])),num(v[4]));
                    default->throw new IllegalArgumentException("move");});}
                if(moves.isEmpty()){emit(f[0],state++,p,f[2],manager);if(parts[0].equals("t"))emit(f[0],state++,p,f[2],manager);continue;}
                var move=Moves.compose(moves);
                if(parts[0].equals("t")){final int n=state++;context.executeTemporarily(move,view->emit(f[0],n,p,f[2],manager));emit(f[0],state++,p,f[2],manager);}
                else{context.execute(move);emit(f[0],state++,p,f[2],manager);}
            }
            } catch(RuntimeException ex) {
                Throwable root=ex;while(root.getCause()!=null)root=root.getCause();
                var message=ex.getClass().getName()+": "+ex.getMessage()+"; root: "+root;
                System.out.println("ERROR\t"+f[0]+"\t"+Base64.getEncoder().encodeToString(message.getBytes(java.nio.charset.StandardCharsets.UTF_8)));
            }
        }}
    }
}
