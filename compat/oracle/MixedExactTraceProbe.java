// TEST-derived opaque runtime observation; Apache-2.0. No production source inspected.
import java.util.*;
import java.util.stream.Collectors;
import ai.timefold.solver.core.api.score.SimpleScore;
import ai.timefold.solver.core.api.score.calculator.EasyScoreCalculator;
import ai.timefold.solver.core.api.solver.SolverFactory;
import ai.timefold.solver.core.config.solver.SolverConfig;
import ai.timefold.solver.core.config.exhaustivesearch.*;
import ai.timefold.solver.core.config.heuristic.selector.entity.EntitySorterManner;
import ai.timefold.solver.core.config.heuristic.selector.value.ValueSorterManner;
import ai.timefold.solver.core.config.score.director.ScoreDirectorFactoryConfig;
import ai.timefold.solver.core.config.solver.termination.TerminationConfig;
import ai.timefold.solver.core.impl.solver.DefaultSolver;
import ai.timefold.solver.core.impl.phase.event.PhaseLifecycleListenerAdapter;
import ai.timefold.solver.core.impl.phase.scope.*;
import ai.timefold.solver.core.testdomain.mixed.singleentity.*;
public class MixedExactTraceProbe {
 static int count;
 static String state(TestdataMixedSolution s) {var b=new StringBuilder();var l=new StringBuilder();for(var e:s.getEntityList()){b.append("(").append(e.getBasicValue()==null?"-":e.getBasicValue().getCode()).append(e.getSecondBasicValue()==null?"-":e.getSecondBasicValue().getCode()).append(")");l.append("[").append(e.getValueList().isEmpty()?"-":e.getValueList().stream().map(v->v.getCode()).collect(Collectors.joining(","))).append("]");}return b.append(l).toString();}
 public static class Calc implements EasyScoreCalculator<TestdataMixedSolution,SimpleScore> {public SimpleScore calculateScore(TestdataMixedSolution s){System.out.println("calc "+(++count)+" "+state(s));int n=0;var used=new HashSet<TestdataMixedOtherValue>();for(var e:s.getEntityList()){int z=e.getValueList().size();if(z==0)n--;else if(z>1)n-=5;if(e.getBasicValue()==e.getSecondBasicValue())n--;if(e.getBasicValue()!=null){if(used.contains(e.getBasicValue()))n-=5;used.add(e.getBasicValue());}}return SimpleScore.of(n);}}
 public static void main(String[] args) {for(var kind:List.of(ExhaustiveSearchType.BRUTE_FORCE,ExhaustiveSearchType.BRANCH_AND_BOUND)){count=0;System.out.println("KIND "+kind);var config=new SolverConfig().withSolutionClass(TestdataMixedSolution.class).withEntityClasses(TestdataMixedEntity.class,TestdataMixedOtherValue.class,TestdataMixedValue.class);config.setPhaseConfigList(List.of(new ExhaustiveSearchPhaseConfig().withExhaustiveSearchType(kind).withNodeExplorationType(kind==ExhaustiveSearchType.BRUTE_FORCE?null:NodeExplorationType.DEPTH_FIRST).withEntitySorterManner(EntitySorterManner.NONE).withValueSorterManner(ValueSorterManner.NONE).withTerminationConfig(new TerminationConfig().withStepCountLimit(37))));config.setScoreDirectorFactoryConfig(new ScoreDirectorFactoryConfig().withEasyScoreCalculatorClass(Calc.class).withInitializingScoreTrend("ONLY_DOWN"));var solver=(DefaultSolver<TestdataMixedSolution>)SolverFactory.<TestdataMixedSolution>create(config).buildSolver();solver.addPhaseLifecycleListener(new PhaseLifecycleListenerAdapter<>() {public void phaseStarted(AbstractPhaseScope<TestdataMixedSolution> p){System.out.println("phase-start "+p.getPhaseIndex()+" count="+count);}public void stepEnded(AbstractStepScope<TestdataMixedSolution> p){System.out.println("step-end "+p.getStepIndex()+" count="+count+" node="+((ai.timefold.solver.core.impl.exhaustivesearch.scope.ExhaustiveSearchStepScope)p).getExpandingNode()+" move="+((ai.timefold.solver.core.impl.exhaustivesearch.scope.ExhaustiveSearchStepScope)p).getExpandingNode().getMove()+" "+state(p.getWorkingSolution()));}public void phaseEnded(AbstractPhaseScope<TestdataMixedSolution> p){System.out.println("phase-end "+p.getPhaseIndex()+" count="+count);}});var s=new TestdataMixedSolution();s.setEntityList(List.of(new TestdataMixedEntity("entity1",1),new TestdataMixedEntity("entity2",2)));s.setValueList(List.of(new TestdataMixedValue("4"),new TestdataMixedValue("5"),new TestdataMixedValue("6")));s.setOtherValueList(List.of(new TestdataMixedOtherValue("1",1),new TestdataMixedOtherValue("2",2),new TestdataMixedOtherValue("3",3)));System.out.println("BEST "+state(solver.solve(s)));}}
}
