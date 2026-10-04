// Test-operation fixture adaptation of ConsecutiveSetTreeTest random methods.
// Apache-2.0; see compat/provenance.json. No Timefold implementation is used.
import java.util.*;
public final class SequenceTraceFixture {
 public static void main(String[] args) {
  for(int mode=0;mode<2;mode++) {
   var random=new Random(1);var counts=new TreeMap<Integer,Integer>();
   for(int i=0;i<1000;i++) {
    int v=random.nextInt(64)-(mode==0?0:32);boolean remove=counts.containsKey(v)&&random.nextDouble()<0.75;
    if(remove)counts.computeIfPresent(v,(k,n)->n==1?null:n-1);else counts.merge(v,1,Integer::sum);
    System.out.println(mode+" "+(remove?"r":"a")+" "+v+" "+(mode==0?v:Math.abs(v)));
   }
  }
 }
}
