// Original development oracle; only pinned Community public APIs are used.
import java.io.*;
import java.util.*;
import ai.timefold.solver.core.api.domain.entity.PlanningEntity;
import ai.timefold.solver.core.api.domain.solution.*;
import ai.timefold.solver.core.api.domain.valuerange.ValueRangeProvider;
import ai.timefold.solver.core.api.domain.variable.PlanningVariable;
import ai.timefold.solver.core.api.score.HardSoftScore;
import ai.timefold.solver.core.api.score.stream.*;
import ai.timefold.solver.core.api.score.stream.bi.BiJoiner;
import ai.timefold.solver.core.api.solver.*;
import ai.timefold.solver.core.config.solver.SolverConfig;

public final class MembershipJoinOracle {
    @PlanningEntity public static class Anchor {
        @PlanningVariable(valueRangeProviderRefs="values") public Integer value=0;
        public Anchor() {}
    }
    public record Left(int id,int first,int second,Integer key,long weight,List<Integer> keys,int mode) {}
    public record Right(int id,int first,int second,Integer key,long weight,List<Integer> keys) {}
    @PlanningSolution public static class Problem {
        @ValueRangeProvider(id="values") @ProblemFactCollectionProperty public List<Integer> values=List.of(0);
        @PlanningEntityCollectionProperty public List<Anchor> anchors=List.of(new Anchor());
        @ProblemFactCollectionProperty public List<Left> left=new ArrayList<>();
        @ProblemFactCollectionProperty public List<Right> right=new ArrayList<>();
        @PlanningScore public HardSoftScore score;
        public Problem() {}
    }
    public static class Provider implements ConstraintProvider {
        @Override public Constraint[] defineConstraints(ConstraintFactory factory) {
            var constraints=new ArrayList<Constraint>();
            for(int mode=0;mode<9;mode++) {
                final int selected=mode;
                BiJoiner<Left,Right> relation=switch(mode%3) {
                    case 0 -> Joiners.containing(Left::keys,Right::key);
                    case 1 -> Joiners.containedIn(Left::key,Right::keys);
                    default -> Joiners.containingAnyOf(Left::keys,Right::keys);
                };
                var left=factory.forEach(Left.class).filter(l->l.mode()==selected);
                var joined=switch(mode/3) {
                    case 0 -> left.join(Right.class,relation);
                    case 1 -> left.join(Right.class,Joiners.equal(Left::first,Right::first),relation);
                    default -> left.join(Right.class,Joiners.equal(Left::first,Right::first),Joiners.equal(Left::second,Right::second),relation);
                };
                constraints.add(joined.reward(HardSoftScore.ONE_HARD).asConstraint("count-"+mode));
                constraints.add(joined.impact(HardSoftScore.ONE_SOFT,(l,r)->l.weight()*r.weight()).asConstraint("weight-"+mode));
            }
            return constraints.toArray(Constraint[]::new);
        }
    }
    static void emit(String id,int state,Problem p,SolutionManager<Problem,HardSoftScore> manager) {
        var score=manager.update(p,SolutionUpdatePolicy.UPDATE_SCORE_ONLY);
        System.out.println(id+"\t"+state+"\t"+score.hardScore()+"\t"+score.softScore()+"\t-");
    }
    record Row(int first,int second,Integer key,long weight,List<Integer> keys) {}
    static Integer key(String s) { return s.equals("n")?null:Integer.valueOf(s); }
    static Row row(String encoded) {
        var f=encoded.split(",",-1);if(f.length!=5)throw new IllegalArgumentException("row arity");
        long weight=Long.parseLong(f[3]);
        if(weight<-1000||weight>1000)throw new IllegalArgumentException("weight bound");
        var keys=new ArrayList<Integer>();
        if(!f[4].isEmpty())for(var k:f[4].split("/",-1))keys.add(key(k));
        return new Row(Integer.parseInt(f[0]),Integer.parseInt(f[1]),key(f[2]),weight,keys);
    }
    public static void main(String[] args)throws Exception {
        var config=new SolverConfig().withSolutionClass(Problem.class).withEntityClasses(Anchor.class).withConstraintProviderClass(Provider.class);
        var manager=SolutionManager.<Problem,HardSoftScore>create(SolverFactory.create(config));
        try(var input=new BufferedReader(new InputStreamReader(System.in))) {
            for(String line;(line=input.readLine())!=null;) {
                var fields=line.split("\t",-1);if(fields.length!=3)throw new IllegalArgumentException("expected case,mode,operations");
                int mode=Integer.parseInt(fields[1]);if(mode<0||mode>=9)throw new IllegalArgumentException("mode");
                var p=new Problem();var left=new TreeMap<Integer,Left>();var right=new TreeMap<Integer,Right>();int state=0;
                emit(fields[0],state++,p,manager);
                if(fields[2].isEmpty())continue;
                for(String encoded:fields[2].split(";",-1)) {
                    var op=encoded.split(":",-1);if(op.length!=4||!(op[0].equals("c")||op[0].equals("t"))||!(op[1].equals("L")||op[1].equals("R")))throw new IllegalArgumentException("operation");
                    int id=Integer.parseInt(op[2]);if(id<0||id>=16)throw new IllegalArgumentException("id bound");
                    var oldLeft=left.get(id);var oldRight=right.get(id);var data=op[3].equals("-")?null:row(op[3]);
                    if(op[1].equals("L")) {if(data==null)left.remove(id);else left.put(id,new Left(id,data.first(),data.second(),data.key(),data.weight(),data.keys(),mode));}
                    else {if(data==null)right.remove(id);else right.put(id,new Right(id,data.first(),data.second(),data.key(),data.weight(),data.keys()));}
                    p.left=new ArrayList<>(left.values());p.right=new ArrayList<>(right.values());emit(fields[0],state++,p,manager);
                    if(op[0].equals("t")) {
                        if(op[1].equals("L")) {if(oldLeft==null)left.remove(id);else left.put(id,oldLeft);}
                        else {if(oldRight==null)right.remove(id);else right.put(id,oldRight);}
                        p.left=new ArrayList<>(left.values());p.right=new ArrayList<>(right.values());emit(fields[0],state++,p,manager);
                    }
                }
            }
        }
    }
}
