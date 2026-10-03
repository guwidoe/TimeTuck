// Development adapter using pinned Community public APIs.
// Domain/scoring fixture adapted from testdomain/shadow/{dynamic_follower,follower},
// Apache-2.0; see compat/provenance.json. Generated inputs and the attributed static boundary are supplied by follower_checks.py.
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

public final class FollowerOracle {
    public interface HasValue { Integer getValue(); }
    @PlanningEntity public static class Leader implements HasValue {
        public int id;
        @PlanningVariable(valueRangeProviderRefs="values") public Integer value;
        public Leader() {}
        public Integer getValue(){return value;}
        public void setValue(Integer value){this.value=value;}
    }
    @PlanningEntity public static class Follower implements HasValue {
        @PlanningVariable(valueRangeProviderRefs="leaders") public Leader leader;
        @ShadowVariable(supplierName="supply") public Integer value;
        public Follower() {}
        public Leader getLeader(){return leader;}
        public void setLeader(Leader leader){this.leader=leader;}
        @ShadowSources("leader.value") public Integer supply(){return leader==null?null:leader.value;}
        public Integer getValue(){return value;}
        public void setValue(Integer value){this.value=value;}
    }
    @PlanningEntity public static class SharedFollower implements HasValue {
        public Leader leader;
        @ShadowVariable(supplierName="supply") public Integer value;
        public SharedFollower() {}
        public Leader getLeader(){return leader;}
        public void setLeader(Leader leader){this.leader=leader;}
        @ShadowSources(value="leader.value",alignmentKey="leader")
        public Integer supply(){return leader==null?null:leader.value;}
        public Integer getValue(){return value;}
        public void setValue(Integer value){this.value=value;}
    }
    @PlanningSolution public static class Problem {
        @ValueRangeProvider(id="values") @ProblemFactCollectionProperty public List<Integer> values=new ArrayList<>();
        @ValueRangeProvider(id="leaders") @PlanningEntityCollectionProperty public List<Leader> leaders=new ArrayList<>();
        @PlanningEntityCollectionProperty public List<Follower> followers=new ArrayList<>();
        @PlanningEntityCollectionProperty public List<SharedFollower> sharedFollowers=new ArrayList<>();
        public boolean shared;
        @PlanningScore public SimpleScore score;
        public Problem() {}
    }
    public static class Provider implements ConstraintProvider {
        public Constraint[] defineConstraints(ConstraintFactory f){
            return new Constraint[]{f.forEach(HasValue.class)
                .groupBy(HasValue::getValue,ConstraintCollectors.count())
                .penalize(SimpleScore.ONE,(v,n)->(long)n*n).asConstraint("spread")};
        }
    }
    static Integer number(String s){return s.equals("-")?null:Integer.valueOf(s);}
    static String text(Integer v){return v==null?"-":v.toString();}
    static String list(List<Integer> values){return values.isEmpty()?"_":values.stream().map(FollowerOracle::text).collect(Collectors.joining(","));}
    static List<Integer> parse(String s){return s.equals("_")?List.of():Arrays.stream(s.split(",",-1)).map(FollowerOracle::number).toList();}
    static void emit(String id,int state,Problem p,SolutionManager<Problem,SimpleScore> manager){
        var score=manager.update(p,SolutionUpdatePolicy.UPDATE_SCORE_ONLY);
        var leaders=p.leaders.stream().map(l->l.value).toList();
        var refs=p.shared?p.sharedFollowers.stream().map(f->f.leader==null?null:f.leader.id).toList():p.followers.stream().map(f->f.leader==null?null:f.leader.id).toList();
        var shadows=p.shared?p.sharedFollowers.stream().map(f->f.value).toList():p.followers.stream().map(f->f.value).toList();
        boolean complete=leaders.stream().noneMatch(Objects::isNull)&&(p.shared||refs.stream().noneMatch(Objects::isNull));
        System.out.println(id+"\t"+state+"\t"+(complete?1:0)+"\t"+score.score()+"\t"+list(leaders)+"\t"+list(refs)+"\t"+list(shadows));
    }
    public static void main(String[] args)throws Exception{
        var meta=PlanningSolutionMetaModel.of(Problem.class,Leader.class,Follower.class,SharedFollower.class);
        var value=meta.genuineEntity(Leader.class).basicVariable("value",Integer.class);
        var leader=meta.genuineEntity(Follower.class).basicVariable("leader",Leader.class);
        var config=new SolverConfig().withSolutionClass(Problem.class).withEntityClasses(Leader.class,Follower.class,SharedFollower.class).withConstraintProviderClass(Provider.class);
        var manager=SolutionManager.<Problem,SimpleScore>create(SolverFactory.create(config));
        try(var input=new BufferedReader(new InputStreamReader(System.in))){
            for(String line;(line=input.readLine())!=null;){
                var f=line.split("\t",-1);if(f.length!=6)throw new IllegalArgumentException("six fields");
                var p=new Problem();int k=Integer.parseInt(f[1]);
                if(!f[5].equals("dynamic")&&!f[5].equals("shared"))throw new IllegalArgumentException("model");
                p.shared=f[5].equals("shared");
                if(k<1||k>32)throw new IllegalArgumentException("value count");
                for(int i=0;i<k;i++)p.values.add(i);
                for(var v:parse(f[2])){var l=new Leader();l.id=p.leaders.size();l.value=v;if(v!=null&&!p.values.contains(v))throw new IllegalArgumentException("value");p.leaders.add(l);}
                if(p.leaders.isEmpty()||p.leaders.size()>64)throw new IllegalArgumentException("leader count");
                for(var ref:parse(f[3])){
                    if(p.shared){var row=new SharedFollower();row.leader=ref==null?null:p.leaders.get(ref);p.sharedFollowers.add(row);}
                    else{var row=new Follower();row.leader=ref==null?null:p.leaders.get(ref);p.followers.add(row);}
                }
                if(p.followers.size()+p.sharedFollowers.size()>64)throw new IllegalArgumentException("follower count");
                SolutionManager.updateShadowVariables(p);
                var context=MoveTester.build(meta).using(p);
                int state=0;emit(f[0],state++,p,manager);
                for(var batch:f[4].split("\\|")){
                    if(batch.isEmpty())continue;
                    var parts=batch.split("@",-1);if(parts.length!=2||!(parts[0].equals("t")||parts[0].equals("c")))throw new IllegalArgumentException("mode");
                    var moves=new ArrayList<Move<Problem>>();
                    if(!parts[1].isEmpty())for(var op:parts[1].split(";")){
                        var bits=op.split(",",-1);if(bits.length!=3)throw new IllegalArgumentException("operation");
                        int i=Integer.parseInt(bits[1]);var v=number(bits[2]);
                        if(bits[0].equals("v")){
                            if(v!=null&&!p.values.contains(v))throw new IllegalArgumentException("value");
                            moves.add(Moves.change(value,p.leaders.get(i),v));
                        }else if(bits[0].equals("r")&&!p.shared)moves.add(Moves.change(leader,p.followers.get(i),v==null?null:p.leaders.get(v)));
                        else throw new IllegalArgumentException("kind");
                    }
                    if(moves.isEmpty()){emit(f[0],state++,p,manager);if(parts[0].equals("t"))emit(f[0],state++,p,manager);continue;}
                    var move=Moves.compose(moves);
                    if(parts[0].equals("t")){final int n=state++;context.executeTemporarily(move,view->emit(f[0],n,p,manager));emit(f[0],state++,p,manager);}
                    else{context.execute(move);emit(f[0],state++,p,manager);}
                }
            }
        }
    }
}
