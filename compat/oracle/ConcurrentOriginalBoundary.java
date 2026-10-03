// Original diagnostic driver for pinned Community test fixtures; no method credit.
// Fixture source files are compiled unchanged from the pinned test tree.
import java.util.*;
import ai.timefold.solver.core.api.solver.*;
import ai.timefold.solver.core.config.solver.SolverConfig;
import ai.timefold.solver.core.preview.api.domain.metamodel.PlanningSolutionMetaModel;
import ai.timefold.solver.core.preview.api.move.test.MoveTester;
import ai.timefold.solver.core.testdomain.shadow.concurrent.*;
public class ConcurrentOriginalBoundary {
    static TestdataConcurrentSolution problem(){
        var v0=new TestdataConcurrentValue("0");var v1=new TestdataConcurrentValue("1");var v2=new TestdataConcurrentValue("2");
        var group=List.of(v0,v1);v0.setConcurrentValueGroup(group);v1.setConcurrentValueGroup(group);
        var a=new TestdataConcurrentEntity("A");var b=new TestdataConcurrentEntity("B");a.setValues(new ArrayList<>(List.of(v2,v1)));
        var p=new TestdataConcurrentSolution();p.setEntities(List.of(a,b));p.setValues(List.of(v0,v1,v2));return p;
    }
    public static void main(String[] args){
        var meta=PlanningSolutionMetaModel.of(TestdataConcurrentSolution.class,TestdataConcurrentEntity.class,TestdataConcurrentValue.class);
        try{
            var p=problem();MoveTester.build(meta).using(p);
            System.out.println("assigned_member_finish="+p.getValues().get(1).getServiceFinishTime());
            var manager=SolutionManager.create(SolverFactory.create(new SolverConfig().withSolutionClass(TestdataConcurrentSolution.class).withEntityClasses(TestdataConcurrentEntity.class,TestdataConcurrentValue.class).withConstraintProviderClass(TestdataConcurrentConstraintProvider.class)));
            System.out.println("score="+manager.update(problem(),SolutionUpdatePolicy.UPDATE_ALL));
        }catch(RuntimeException ex){
            Throwable root=ex;while(root.getCause()!=null)root=root.getCause();
            System.out.println("error="+root.getClass().getName()+":"+root.getMessage());
        }
    }
}
