// Adaptation of ConnectedRangeTrackerTest.testRandomRanges selection
// scaffolding, Apache-2.0, pinned 8b815bc6b5f3eb81feedc94dea0885a93b267762.
// See compat/provenance.json. Executes the test input machinery only; no native expected results.
package ai.timefold.solver.core.impl.score.stream.collector.connected_ranges;
import java.util.*;
public class RangeTraceFixture {
 static final class R {
  final int start,end; R(int a,int b){start=a;end=b;} int start(){return start;} int end(){return end;}
  public boolean equals(Object o){return o instanceof R r && start==r.start && end==r.end;}
  public int hashCode(){return Objects.hash(start,end);}
 }
 public static void main(String[] args){var random=new Random(1);
  for(int i=0;i<100;i++) {var points=new TreeSet<RangeSplitPoint<R,Integer>>();
   for(int j=0;j<100;j++){int a=random.nextInt(5),b=a+random.nextInt(5);var range=new Range<>(new R(a,b),R::start,R::end);
    points.add(range.getStartSplitPoint());points.add(range.getEndSplitPoint());
    var start=points.floor(range.getStartSplitPoint());var end=points.floor(range.getEndSplitPoint());
    if(start.startpointRangeToCountMap==null)start.createCollections();if(end.endpointRangeToCountMap==null)end.createCollections();
    boolean remove=start.containsRangeStarting(range)&&random.nextBoolean();
    if(remove){start.removeRangeStartingAtSplitPoint(range);end.removeRangeEndingAtSplitPoint(range);if(start.isEmpty())points.remove(start);if(end.isEmpty())points.remove(end);}
    else{start.addRangeStartingAtSplitPoint(range);end.addRangeEndingAtSplitPoint(range);}
    System.out.println(i+" "+(remove?"r":"a")+" "+a+" "+b);
   }
  }
 }
}
