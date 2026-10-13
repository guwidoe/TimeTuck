// TEST fixture/helper reuse: Apache-2.0. Reflection targets ONLY the pinned
// TEST class/private fixture helpers. No production reflection or body inspection.
package ai.timefold.solver.core.impl.heuristic.selector.move.generic.list.kopt;
import java.util.*;
import java.lang.reflect.*;
import ai.timefold.solver.core.impl.domain.variable.ListVariableState;
import static ai.timefold.solver.core.impl.heuristic.selector.move.generic.list.kopt.KOptUtils.*;
public class PartnerTailExperiment {
 @SuppressWarnings("unchecked")
 static void run(String fixture,String mode)throws Exception {
  var test=new KOptListMoveIteratorTest();
  var create=KOptListMoveIteratorTest.class.getDeclaredMethod("createMockKOptListMoveIterator",int.class,int.class,int[].class);create.setAccessible(true);
  Object mock=create.invoke(test,2,6,new int[]{1,1,1,1});
  var setup=KOptListMoveIteratorTest.class.getDeclaredMethod("setupValidNonsequential4OptMove",mock.getClass(),Object[].class);setup.setAccessible(true);
  Object[] owners=fixture.equals("same")?new Object[]{"e1","e1","e1","e1","e1"}:new Object[]{"e1","e2","e1","e2","e1"};setup.invoke(test,mock,owners);
  var iteratorField=mock.getClass().getDeclaredField("kOptListMoveIterator");iteratorField.setAccessible(true);
  var stateField=mock.getClass().getDeclaredField("listVariableState");stateField.setAccessible(true);
  var iterator=(KOptListMoveIterator<Object,Object>)iteratorField.get(mock);
  var state=(ListVariableState<Object,Object,Object>)stateField.get(mock);
  var original=((SelectorBasedKOptListMove<Object>)iterator.createUpcomingSelection()).getDescriptor();
  Object[] removed=original.removedEdges().clone();int[] partners=original.addedEdgeToOtherEndpoint().clone();
  if(mode.equals("active13"))partners=Arrays.copyOf(partners,removed.length);
  if(mode.equals("tail_changed")){partners[13]=14;partners[14]=13;partners[15]=16;partners[16]=15;}
  if(mode.equals("active_control")){
   int[] order=original.removedEdgeIndexToTourOrder();int active=2*original.k();
   for(int pair=0;pair<original.k();pair++){int a=order[2*pair+2];int b=order[(2*pair+2)%active+1];partners[a]=b;partners[b]=a;}
  }
  // Both callback factories and the descriptor constructor are used verbatim
  // in the pinned TEST helper. Every variant constructs a NEW descriptor with
  // a NEW original fixture context; no production cached object is edited.
  Object[] picked=Arrays.copyOfRange(removed,1,removed.length);
  var fresh=new KOptDescriptor<Object>(removed,partners,getMultiEntitySuccessorFunction(picked,state),getMultiEntityBetweenPredicate(picked,state));
  System.out.println("BEGIN fixture="+fixture+" mode="+mode);
  System.out.println("FRESH k="+fresh.k()+" removed="+Arrays.toString(fresh.removedEdges())+" partners="+Arrays.toString(fresh.addedEdgeToOtherEndpoint()));
  System.out.println("ORDER "+Arrays.toString(fresh.removedEdgeIndexToTourOrder())+" INVERSE "+Arrays.toString(fresh.inverseRemovedEdgeIndexToTourOrder()));
  try{var cycles=getCyclesForPermutation(fresh);System.out.println("CYCLES count="+cycles.cycleCount()+" ids="+Arrays.toString(cycles.indexToCycleIdentifier()));}catch(Exception ex){System.out.println("CYCLES_ERROR "+ex.getClass().getName()+" "+ex.getMessage());}
  try{var converted=fresh.getKOptListMove(state).getDescriptor();System.out.println("CONVERTED k="+converted.k()+" removed="+Arrays.toString(converted.removedEdges())+" partners="+Arrays.toString(converted.addedEdgeToOtherEndpoint()));var cycles=getCyclesForPermutation(converted);System.out.println("CONVERTED_CYCLES count="+cycles.cycleCount()+" ids="+Arrays.toString(cycles.indexToCycleIdentifier()));}catch(Exception ex){System.out.println("CONVERT_ERROR "+ex.getClass().getName()+" "+ex.getMessage());}
  System.out.println("END fixture="+fixture+" mode="+mode);
 }
 public static void main(String[]args)throws Exception{for(String fixture:new String[]{"same","different"})for(String mode:new String[]{"full17","active13","tail_changed","active_control"})run(fixture,mode);}
}
