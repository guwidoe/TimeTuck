// Collection follower domain/supplier fixture adapted from Timefold Community
// testdomain/shadow/follower_set, Apache-2.0. Reuses attributed FollowerOracle
// leader/interface/provider fixtures. See compat/provenance.json.
// Generated comparison inputs are original.
import java.io.*;
import java.util.*;
import ai.timefold.solver.core.api.domain.entity.PlanningEntity;
import ai.timefold.solver.core.api.domain.solution.*;
import ai.timefold.solver.core.api.domain.valuerange.ValueRangeProvider;
import ai.timefold.solver.core.api.domain.variable.*;
import ai.timefold.solver.core.api.score.SimpleScore;
import ai.timefold.solver.core.api.solver.*;
import ai.timefold.solver.core.config.solver.SolverConfig;
import ai.timefold.solver.core.preview.api.domain.metamodel.PlanningSolutionMetaModel;
import ai.timefold.solver.core.preview.api.move.Move;
import ai.timefold.solver.core.preview.api.move.builtin.Moves;
import ai.timefold.solver.core.preview.api.move.test.MoveTester;

public final class CollectionFollowerOracle {
    @PlanningEntity public static class Follower implements FollowerOracle.HasValue {
        public List<FollowerOracle.Leader> leaders;
        public List<String> codes;
        @ShadowVariable(supplierName="supply") public Integer value;
        public Follower() {}
        public List<FollowerOracle.Leader> getLeaders(){return leaders;}
        public void setLeaders(List<FollowerOracle.Leader> leaders){this.leaders=leaders;}
        public Integer getValue(){return value;}
        public void setValue(Integer value){this.value=value;}
        @ShadowSources("leaders[].value") public Integer supply(){
            Integer best=null;
            for(var leader:leaders)if(leader.value!=null&&(best==null||codes.get(leader.value).compareTo(codes.get(best))<0))best=leader.value;
            return best;
        }
    }
    @PlanningSolution public static class Problem {
        @ValueRangeProvider(id="values") @ProblemFactCollectionProperty public List<Integer> values=new ArrayList<>();
        @PlanningEntityCollectionProperty public List<FollowerOracle.Leader> leaders=new ArrayList<>();
        @PlanningEntityCollectionProperty public List<Follower> followers=new ArrayList<>();
        @PlanningScore public SimpleScore score;
        public Problem() {}
    }
    static void emit(String id,int state,Problem p,SolutionManager<Problem,SimpleScore> manager){
        var score=manager.update(p,SolutionUpdatePolicy.UPDATE_SCORE_ONLY);
        var leaders=p.leaders.stream().map(l->l.value).toList();
        var shadows=p.followers.stream().map(f->f.value).toList();
        System.out.println(id+"\t"+state+"\t"+(leaders.stream().noneMatch(Objects::isNull)?1:0)+"\t"+score.score()+"\t"+FollowerOracle.list(leaders)+"\t"+FollowerOracle.list(shadows));
    }
    public static void main(String[] args)throws Exception{
        var meta=PlanningSolutionMetaModel.of(Problem.class,FollowerOracle.Leader.class,Follower.class);
        var value=meta.genuineEntity(FollowerOracle.Leader.class).basicVariable("value",Integer.class);
        var config=new SolverConfig().withSolutionClass(Problem.class).withEntityClasses(FollowerOracle.Leader.class,Follower.class).withConstraintProviderClass(FollowerOracle.Provider.class);
        var manager=SolutionManager.<Problem,SimpleScore>create(SolverFactory.create(config));
        try(var input=new BufferedReader(new InputStreamReader(System.in))){
            for(String line;(line=input.readLine())!=null;){
                var f=line.split("\t",-1);if(f.length!=5)throw new IllegalArgumentException("five fields");
                var p=new Problem();var codes=Arrays.asList(f[1].split(",",-1));
                if(codes.size()>32)throw new IllegalArgumentException("code count");
                for(int i=0;i<codes.size();i++)p.values.add(i);
                for(var v:FollowerOracle.parse(f[2])){var l=new FollowerOracle.Leader();l.id=p.leaders.size();l.value=v;if(v!=null&&!p.values.contains(v))throw new IllegalArgumentException("value");p.leaders.add(l);}
                if(p.leaders.isEmpty()||p.leaders.size()>64)throw new IllegalArgumentException("leader count");
                if(!f[3].equals("_"))for(var group:f[3].split(";",-1)){
                    var row=new Follower();row.codes=codes;row.leaders=new ArrayList<>();
                    for(var ref:FollowerOracle.parse(group)){
                        if(ref==null)throw new IllegalArgumentException("null collection member");
                        row.leaders.add(p.leaders.get(ref));
                    }
                    if(row.leaders.isEmpty())throw new IllegalArgumentException("empty dependency collection");
                    p.followers.add(row);
                }
                if(p.followers.size()>64)throw new IllegalArgumentException("follower count");
                SolutionManager.updateShadowVariables(p);var context=MoveTester.build(meta).using(p);
                int state=0;emit(f[0],state++,p,manager);
                for(var batch:f[4].split("\\|")){
                    if(batch.isEmpty())continue;
                    var parts=batch.split("@",-1);if(parts.length!=2||!(parts[0].equals("t")||parts[0].equals("c")))throw new IllegalArgumentException("mode");
                    var moves=new ArrayList<Move<Problem>>();
                    if(!parts[1].isEmpty())for(var op:parts[1].split(";")){
                        var bits=op.split(",",-1);if(bits.length!=2)throw new IllegalArgumentException("operation");
                        int i=Integer.parseInt(bits[0]);var v=FollowerOracle.number(bits[1]);
                        if(v!=null&&!p.values.contains(v))throw new IllegalArgumentException("value");
                        moves.add(Moves.change(value,p.leaders.get(i),v));
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
