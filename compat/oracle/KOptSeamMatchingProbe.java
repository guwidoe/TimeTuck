// TEST fixture/helper adaptations: Apache-2.0; see compat/provenance.json.
// Original opaque descriptor-order observation, no implementation inspection.
package ai.timefold.solver.core.impl.heuristic.selector.move.generic.list.kopt;
import java.util.*;
import java.util.function.Function;
import static org.mockito.Mockito.*;
import static ai.timefold.solver.core.impl.heuristic.selector.move.generic.list.kopt.KOptUtils.*;
import ai.timefold.solver.core.api.solver.SolutionManager;
import ai.timefold.solver.core.testdomain.list.*;
import ai.timefold.solver.core.impl.score.director.easy.EasyScoreDirectorFactory;
import ai.timefold.solver.core.impl.domain.variable.ListVariableState;
import ai.timefold.solver.core.api.score.SimpleScore;
import ai.timefold.solver.core.config.solver.EnvironmentMode;
public class KOptSeamMatchingProbe {
 static IdentityHashMap<Object,Integer> ids;
 static int[] numbers(Object[] values){return Arrays.stream(values).mapToInt(v->v==null?0:ids.get(v)).toArray();}
 static int[] row(TestdataListEntity entity){return numbers(entity.getValueList().toArray());}
 static void run(String name,int[] aa,int[] bb,int pin,int[] removed,int[] added,boolean reverse){
  ids=new IdentityHashMap<>();var values=new LinkedHashMap<Integer,TestdataListValue>();for(int i:aa)values.put(i,new TestdataListValue(""+i));for(int i:bb)values.put(i,new TestdataListValue(""+i));values.forEach((i,v)->ids.put(v,i));
  var a=new TestdataListEntity("A",Arrays.stream(aa).mapToObj(values::get).toArray(TestdataListValue[]::new));var b=new TestdataListEntity("B",Arrays.stream(bb).mapToObj(values::get).toArray(TestdataListValue[]::new));
  var solution=new TestdataListSolution();solution.setEntityList(reverse?List.of(b,a):List.of(a,b));solution.setValueList(new ArrayList<>(values.values()));SolutionManager.updateShadowVariables(solution);
  var variable=TestdataListEntity.buildVariableDescriptorForValueList();var director=new EasyScoreDirectorFactory<TestdataListSolution,SimpleScore>(variable.getEntityDescriptor().getSolutionDescriptor(),s->SimpleScore.ZERO,EnvironmentMode.PHASE_ASSERT).buildScoreDirector();director.setWorkingSolution(solution);
  var meta=spy(variable);doReturn(pin).when(meta).getFirstUnpinnedIndex(a);doReturn(0).when(meta).getFirstUnpinnedIndex(b);
  var state=spy(director.getListVariableState(variable));doReturn(meta).when(state).getSourceVariableDescriptor();
  Object[] picked=Arrays.stream(removed).mapToObj(values::get).toArray();Object[] tour=new Object[picked.length+1];System.arraycopy(picked,0,tour,1,picked.length);int[] incl=new int[tour.length];
  for(int i=0;i<removed.length;i++){int where=0;while(added[where]!=removed[i])where++;int partner=added[where^1];int at=0;while(removed[at]!=partner)at++;incl[i+1]=at+1;}
  var descriptor=new KOptDescriptor<Object>(tour,incl,getMultiEntitySuccessorFunction(picked,state),getMultiEntityBetweenPredicate(picked,state));
  var move=descriptor.getKOptListMove(state);var result=move.getDescriptor();
  String before="\"removed\":"+Arrays.toString(numbers(descriptor.removedEdges()))+",\"order\":"+Arrays.toString(descriptor.removedEdgeIndexToTourOrder())+",\"inverse\":"+Arrays.toString(descriptor.inverseRemovedEdgeIndexToTourOrder())+",\"partners\":"+Arrays.toString(descriptor.addedEdgeToOtherEndpoint());
  String converted="\"removed\":"+Arrays.toString(numbers(result.removedEdges()))+",\"order\":"+Arrays.toString(result.removedEdgeIndexToTourOrder())+",\"inverse\":"+Arrays.toString(result.inverseRemovedEdgeIndexToTourOrder())+",\"partners\":"+Arrays.toString(result.addedEdgeToOtherEndpoint());
  System.out.println("PLAN "+name+" "+move.toString());
  if(!move.isMoveDoable(director)){System.out.println("{\"case\":\""+name+"\",\"doable\":false}");director.close();return;}
  move.execute(director);director.triggerVariableListeners();
  System.out.println("{\"case\":\""+name+"\",\"prefix\":"+pin+",\"before\": {"+before+"},\"converted\": {"+converted+"},\"rows\":["+Arrays.toString(row(a))+","+Arrays.toString(row(b))+"]}");director.close();
 }
 static void matchings(List<Integer> available,List<Integer> pairs,List<int[]> out){
  if(available.isEmpty()){out.add(pairs.stream().mapToInt(i->i).toArray());return;}
  int first=available.get(0);
  for(int i=1;i<available.size();i++){
   int partner=available.get(i);var next=new ArrayList<>(available);next.remove(i);next.remove(0);
   pairs.add(first);pairs.add(partner);matchings(next,pairs,out);pairs.remove(pairs.size()-1);pairs.remove(pairs.size()-1);
  }
 }
 public static void main(String[] args){
  var rem=new int[]{6,1,2,3,4,5};var all=new ArrayList<int[]>();matchings(List.of(1,2,3,4,5,6),new ArrayList<>(),all);
  int i=0;
  for(var add:all){
   for(int variant=0;variant<2;variant++){
    String name="matching-"+i+"-"+variant;
    try{run(name,variant==0?new int[]{1,2,3,6}:new int[]{1,7,8,2,3,9,10,6},variant==0?new int[]{4,5}:new int[]{4,5,11,12},0,rem,add,false);}
    catch(RuntimeException e){System.out.println("{\"case\":\""+name+"\",\"error\":\""+e.getClass().getName()+"\"}");}
   }
   i++;
  }
  var add=new int[]{1,3,2,5,4,6};
  try{run("orientation-first-reversed",new int[]{1,7,8,2,3,9,10,6},new int[]{4,5,11,12},0,new int[]{1,6,2,3,4,5},add,false);}catch(RuntimeException e){System.out.println(e);}
  try{run("orientation-middle-reversed",new int[]{1,7,8,2,3,9,10,6},new int[]{4,5,11,12},0,new int[]{6,1,3,2,4,5},add,false);}catch(RuntimeException e){System.out.println(e);}
 }
}
