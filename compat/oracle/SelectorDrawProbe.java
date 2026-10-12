// Adapted pinned Community TEST setup, Apache-2.0. Provenance in compat/provenance.json.
// This probe observes bounded draws/moves, without inspecting production implementation.
import java.util.*;
import static org.mockito.Mockito.*;
import static org.mockito.ArgumentMatchers.*;
import ai.timefold.solver.core.api.score.SimpleScore;
import ai.timefold.solver.core.config.solver.EnvironmentMode;
import ai.timefold.solver.core.config.heuristic.selector.common.SelectionCacheType;
import ai.timefold.solver.core.config.heuristic.selector.entity.EntitySelectorConfig;
import ai.timefold.solver.core.config.heuristic.selector.value.ValueSelectorConfig;
import ai.timefold.solver.core.config.heuristic.selector.list.DestinationSelectorConfig;
import ai.timefold.solver.core.impl.heuristic.HeuristicConfigPolicy;
import ai.timefold.solver.core.impl.heuristic.selector.list.DestinationSelectorFactory;
import ai.timefold.solver.core.impl.heuristic.selector.move.generic.list.ListChangeMoveSelector;
import ai.timefold.solver.core.impl.heuristic.selector.value.FromEntityPropertyValueSelector;
import ai.timefold.solver.core.impl.heuristic.selector.value.decorator.IterableFromEntityPropertyValueSelector;
import ai.timefold.solver.core.impl.heuristic.selector.value.mimic.MimicRecordingValueSelector;
import ai.timefold.solver.core.impl.heuristic.selector.common.decorator.SelectionFilter;
import ai.timefold.solver.core.impl.score.director.ScoreDirector;
import ai.timefold.solver.core.impl.score.director.easy.EasyScoreDirectorFactory;
import ai.timefold.solver.core.impl.phase.scope.AbstractPhaseScope;
import ai.timefold.solver.core.impl.solver.scope.SolverScope;
import ai.timefold.solver.core.impl.solver.random.MockRandomSource;
import ai.timefold.solver.core.impl.solver.ClassInstanceCache;
import ai.timefold.solver.core.testdomain.list.valuerange.*;
public class SelectorDrawProbe {
 static class Draws extends Random {
  Draws(long seed){super(seed);} final List<String> draws=new ArrayList<>();
  @Override protected int next(int bits){int v=super.next(bits);if(draws!=null)draws.add("b"+bits+":"+v);return v;}
  @Override public long nextLong(long bound){long v=super.nextLong(bound);draws.add("L"+bound+":"+v);return v;}
  @Override public int nextInt(int bound){int v=super.nextInt(bound);draws.add(bound+":"+v);return v;}
 }
 static class Fixed extends Draws {
  Fixed(int main,int category,int rank){super(0);this.main=main;this.category=category;this.rank=rank;}
  final int main,category,rank;int call=0;
  @Override public int nextInt(int n){int v=(call++%3==0?main:call%3==2?category:rank)%n;draws.add(n+":"+v);return v;}
 }
 public static class IgnoreB implements SelectionFilter<TestdataListEntityProvidingSolution,TestdataListEntityProvidingValue> {
  public boolean accept(ScoreDirector<TestdataListEntityProvidingSolution> d,TestdataListEntityProvidingValue v){return !v.getEntity().getCode().equals("B");}
 }
 @SuppressWarnings({"unchecked","rawtypes"})
 public static void main(String[] args) {
  if(args.length>0&&args[0].equals("optional")){optional(args);return;}
  boolean filtered=args.length>0&&args[0].equals("filtered");long seed=args.length>1?Long.parseLong(args[1]):0;
  var v1=new TestdataListEntityProvidingValue("1");var v2=new TestdataListEntityProvidingValue("2");var v3=new TestdataListEntityProvidingValue("3");
  var a=new TestdataListEntityProvidingEntity("A",filtered?List.of(v1,v2,v3):List.of(v1,v2),List.of(v2,v1));
  var b=new TestdataListEntityProvidingEntity("B",List.of(v2,v3),List.of(v3));
  var solution=new TestdataListEntityProvidingSolution();solution.setEntityList(List.of(a,b));
  var descriptor=TestdataListEntityProvidingSolution.buildSolutionDescriptor();
  var director=new EasyScoreDirectorFactory<>(descriptor,s->SimpleScore.ZERO,EnvironmentMode.PHASE_ASSERT).buildScoreDirector();director.setWorkingSolution(solution);
  var entity=descriptor.findEntityDescriptor(TestdataListEntityProvidingEntity.class);var variable=entity.getListVariableDescriptor();
  var from=new FromEntityPropertyValueSelector<>(variable.getValueRangeDescriptor(),null,true);
  var recording=new MimicRecordingValueSelector<>(new IterableFromEntityPropertyValueSelector<>(from,true));
  var config=new DestinationSelectorConfig().withEntitySelectorConfig(new EntitySelectorConfig().withEntityClass(entity.getEntityClass())).withValueSelectorConfig(new ValueSelectorConfig().withVariableName(variable.getVariableName()));
  if(filtered)config.getValueSelectorConfig().setFilterClass(IgnoreB.class);
  var policy=mock(HeuristicConfigPolicy.class);doReturn(descriptor).when(policy).getSolutionDescriptor();doReturn(recording).when(policy).getValueMimicRecorder(any());doReturn(ClassInstanceCache.create()).when(policy).getClassInstanceCache();
  var dest=DestinationSelectorFactory.<TestdataListEntityProvidingSolution>create(config).buildDestinationSelector(policy,SelectionCacheType.JUST_IN_TIME,true,"any",false);
  var selector=new ListChangeMoveSelector<>(recording,dest,true);
  Draws random=args.length>2?new Fixed(Integer.parseInt(args[1]),Integer.parseInt(args[2]),Integer.parseInt(args[3])):new Draws(seed);var scope=new SolverScope<TestdataListEntityProvidingSolution>();scope.setScoreDirector(director);scope.setWorkingRandom(new MockRandomSource(random));selector.solvingStarted(scope);
  var phase=mock(AbstractPhaseScope.class);when(phase.getSolverScope()).thenReturn(scope);when(phase.getScoreDirector()).thenReturn(director);when(phase.getWorkingSolution()).thenReturn(solution);selector.phaseStarted(phase);
  var iterator=selector.iterator();
  for(int i=0;i<(args.length>2?1:12);i++){random.draws.clear();boolean has=iterator.hasNext();var move=iterator.next();System.out.println(i+"\t"+has+"\t"+move+"\t"+String.join(",",random.draws));}
 }
 @SuppressWarnings({"unchecked","rawtypes"})
 static void optional(String[] args) {
  var v1=new ai.timefold.solver.core.testdomain.TestdataValue("1");var v2=new ai.timefold.solver.core.testdomain.TestdataValue("2");var v3=new ai.timefold.solver.core.testdomain.TestdataValue("3");
  var a=new ai.timefold.solver.core.testdomain.list.valuerange.unassignedvar.TestdataListUnassignedEntityProvidingEntity("A",List.of(v1,v2));a.setValueList(List.of(v2,v1));
  var b=new ai.timefold.solver.core.testdomain.list.valuerange.unassignedvar.TestdataListUnassignedEntityProvidingEntity("B",List.of(v2,v3));b.setValueList(List.of(v3));
  var solution=new ai.timefold.solver.core.testdomain.list.valuerange.unassignedvar.TestdataListUnassignedEntityProvidingSolution();solution.setEntityList(List.of(a,b));
  var descriptor=ai.timefold.solver.core.testdomain.list.valuerange.unassignedvar.TestdataListUnassignedEntityProvidingSolution.buildSolutionDescriptor();
  var director=new EasyScoreDirectorFactory<>(descriptor,s->SimpleScore.ZERO,EnvironmentMode.PHASE_ASSERT).buildScoreDirector();director.setWorkingSolution(solution);
  var entity=descriptor.findEntityDescriptor(a.getClass());var variable=entity.getListVariableDescriptor();
  var recording=new MimicRecordingValueSelector<>(new IterableFromEntityPropertyValueSelector<>(new FromEntityPropertyValueSelector<>(variable.getValueRangeDescriptor(),null,true),true));
  var config=new DestinationSelectorConfig().withEntitySelectorConfig(new EntitySelectorConfig().withEntityClass(entity.getEntityClass())).withValueSelectorConfig(new ValueSelectorConfig().withVariableName(variable.getVariableName()));
  var policy=mock(HeuristicConfigPolicy.class);doReturn(descriptor).when(policy).getSolutionDescriptor();doReturn(recording).when(policy).getValueMimicRecorder(any());doReturn(ClassInstanceCache.create()).when(policy).getClassInstanceCache();
  var dest=DestinationSelectorFactory.create(config).buildDestinationSelector(policy,SelectionCacheType.JUST_IN_TIME,true,"any",false);
  var selector=new ListChangeMoveSelector(recording,dest,true);var random=new Draws(args.length>1?Long.parseLong(args[1]):0);
  var scope=new SolverScope();scope.setScoreDirector(director);scope.setWorkingRandom(new MockRandomSource(random));selector.solvingStarted(scope);
  var phase=mock(AbstractPhaseScope.class);when(phase.getSolverScope()).thenReturn(scope);when(phase.getScoreDirector()).thenReturn(director);when(phase.getWorkingSolution()).thenReturn(solution);selector.phaseStarted(phase);
  var iterator=selector.iterator();for(int i=0;i<12;i++){random.draws.clear();boolean has=iterator.hasNext();var move=iterator.next();System.out.println(i+"\t"+has+"\t"+move+"\t"+String.join(",",random.draws));}
 }

}
