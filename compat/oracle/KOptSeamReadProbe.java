// TEST fixture/helper adaptations: Apache-2.0; see compat/provenance.json.
// Original observation of public List mutations on owned TEST data.
// No production implementation, bytecode body or private fields inspected.
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
public class KOptSeamReadProbe {
 static boolean recording; static String stage="OFF";
 static final class TraceRow extends AbstractList<TestdataListValue> implements RandomAccess {
  final String owner; final List<TestdataListValue> data;
  TraceRow(String owner,List<TestdataListValue> values){this.owner=owner;this.data=new ArrayList<>(values);}
  public int size(){return data.size();}
  public TestdataListValue get(int index){var v=data.get(index);if(recording)System.out.println("READ "+stage+" "+owner+" "+index+" "+ids.get(v));return v;}
  public List<TestdataListValue> subList(int from,int to){if(recording)System.out.println("RANGE "+stage+" "+owner+" "+from+" "+to);return super.subList(from,to);}
  void log(String operation,int index,Object old,Object value){if(false)System.out.println("WRITE "+owner+" "+operation+" "+index+" "+(old==null?0:ids.get(old))+" "+(value==null?0:ids.get(value))+" "+Arrays.toString(numbers(data.toArray())));}
  public TestdataListValue set(int index,TestdataListValue value){var old=data.set(index,value);log("set",index,old,value);return old;}
  public void add(int index,TestdataListValue value){data.add(index,value);modCount++;log("add",index,null,value);}
  public TestdataListValue remove(int index){var old=data.remove(index);modCount++;log("remove",index,old,null);return old;}
  public void clear(){data.clear();modCount++;log("clear",0,null,null);}
 }
 static IdentityHashMap<Object,Integer> ids;
 static int[] numbers(Object[] values){return Arrays.stream(values).mapToInt(v->v==null?0:ids.get(v)).toArray();}
 static int[] row(TestdataListEntity entity){return numbers(entity.getValueList().toArray());}
 static void run(String name,int[] aa,int[] bb,int pin,int[] removed,int[] added,boolean reverse){
  ids=new IdentityHashMap<>();var values=new LinkedHashMap<Integer,TestdataListValue>();for(int i:aa)values.put(i,new TestdataListValue(""+i));for(int i:bb)values.put(i,new TestdataListValue(""+i));values.forEach((i,v)->ids.put(v,i));
  var a=new TestdataListEntity("A",Arrays.stream(aa).mapToObj(values::get).toArray(TestdataListValue[]::new));var b=new TestdataListEntity("B",Arrays.stream(bb).mapToObj(values::get).toArray(TestdataListValue[]::new));
  a.setValueList(new TraceRow("A",a.getValueList()));b.setValueList(new TraceRow("B",b.getValueList()));
  var solution=new TestdataListSolution();solution.setEntityList(reverse?List.of(b,a):List.of(a,b));solution.setValueList(new ArrayList<>(values.values()));SolutionManager.updateShadowVariables(solution);
  var variable=TestdataListEntity.buildVariableDescriptorForValueList();var director=new EasyScoreDirectorFactory<TestdataListSolution,SimpleScore>(variable.getEntityDescriptor().getSolutionDescriptor(),s->SimpleScore.ZERO,EnvironmentMode.PHASE_ASSERT).buildScoreDirector();director.setWorkingSolution(solution);
  var meta=spy(variable);doReturn(pin).when(meta).getFirstUnpinnedIndex(a);doReturn(0).when(meta).getFirstUnpinnedIndex(b);
  var state=spy(director.getListVariableState(variable));doReturn(meta).when(state).getSourceVariableDescriptor();
  doAnswer(invocation->{Object v=invocation.getArgument(0);Object answer=invocation.callRealMethod();if(recording)System.out.println("INDEX "+stage+" "+ids.get(v)+" "+answer);return answer;}).when(state).getIndexOrFail(any());
  doAnswer(invocation->{Object v=invocation.getArgument(0);Object answer=invocation.callRealMethod();if(recording)System.out.println("OWNER "+stage+" "+ids.get(v)+" "+(answer==a?"A":answer==b?"B":"other"));return answer;}).when(state).getInverseSingleton(any());
  System.out.println("BEGIN "+name);stage="CONVERT";recording=true;
  Object[] picked=Arrays.stream(removed).mapToObj(values::get).toArray();Object[] tour=new Object[picked.length+1];System.arraycopy(picked,0,tour,1,picked.length);int[] incl=new int[tour.length];
  for(int i=0;i<removed.length;i++){int where=0;while(added[where]!=removed[i])where++;int partner=added[where^1];int at=0;while(removed[at]!=partner)at++;incl[i+1]=at+1;}
  var descriptor=new KOptDescriptor<Object>(tour,incl,getMultiEntitySuccessorFunction(picked,state),getMultiEntityBetweenPredicate(picked,state));
  var move=descriptor.getKOptListMove(state);var result=move.getDescriptor();
  String before="\"removed\":"+Arrays.toString(numbers(descriptor.removedEdges()))+",\"order\":"+Arrays.toString(descriptor.removedEdgeIndexToTourOrder())+",\"inverse\":"+Arrays.toString(descriptor.inverseRemovedEdgeIndexToTourOrder())+",\"partners\":"+Arrays.toString(descriptor.addedEdgeToOtherEndpoint());
  String converted="\"removed\":"+Arrays.toString(numbers(result.removedEdges()))+",\"order\":"+Arrays.toString(result.removedEdgeIndexToTourOrder())+",\"inverse\":"+Arrays.toString(result.inverseRemovedEdgeIndexToTourOrder())+",\"partners\":"+Arrays.toString(result.addedEdgeToOtherEndpoint());
  System.out.println("PLAN "+name+" "+move.toString());
  stage="EXECUTE";
  move.execute(director);recording=false;director.triggerVariableListeners();
  recording=false;
  System.out.println("{\"case\":\""+name+"\",\"prefix\":"+pin+",\"before\": {"+before+"},\"converted\": {"+converted+"},\"rows\":["+Arrays.toString(row(a))+","+Arrays.toString(row(b))+"]}");director.close();
 }
 public static void main(String[] args){
  var rem=new int[]{6,1,2,3,4,5};var add=new int[]{1,3,2,5,4,6};
  run("base-read-discriminator",new int[]{1,2,3,6},new int[]{4,5},0,rem,add,false);
  run("interior-read-discriminator",new int[]{1,7,8,2,3,9,10,6},new int[]{4,5,11,12},0,rem,add,false);
 }
}
