// Original runner for unchanged pinned Community TEST source.
// Apache-2.0 fixture provenance: compat/provenance.json.
package ai.timefold.solver.core.impl.heuristic.selector.move.generic.list.kopt;
public class KOptIteratorSpecRunner {
 public static void main(String[] args)throws Exception {
  for(String name:new String[]{"testSequentialKOptOnSameEntity","testSequentialKOptOnDifferentEntities","testNonsequentialKOptOnSameEntity","testNonsequentialKOptOnDifferentEntity"}) {
   try{var test=new KOptListMoveIteratorTest();KOptListMoveIteratorTest.class.getDeclaredMethod(name).invoke(test);System.out.println("PASS "+name);}catch(java.lang.reflect.InvocationTargetException e){System.out.println("FAIL "+name+" "+e.getCause());}
  }
 }
}
