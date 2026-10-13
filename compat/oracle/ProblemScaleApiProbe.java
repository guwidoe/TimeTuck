// TEST-visible Community statistics API probe. Adapted constructors from
// ProblemSizeStatisticsTest; Apache-2.0. See compat/provenance.json.
import java.util.Collections;
import ai.timefold.solver.core.api.solver.ProblemSizeStatistics;
import ai.timefold.solver.core.impl.util.MathUtils;
public class ProblemScaleApiProbe {
    public static void main(String[] args) {
        System.out.println("precision=" + MathUtils.LOG_PRECISION);
        for (double log : new double[]{0.0000004,0.0000005,0.0000006,-0.0000005,Double.NaN,Double.NEGATIVE_INFINITY,Double.POSITIVE_INFINITY}) {
            var s=new ProblemSizeStatistics(0L,Collections.emptySortedMap(),0L,0L,Collections.emptySortedMap(),log);
            System.out.println("log="+log+","+s.approximateProblemScaleLogAsFixedPointLong());
        }
        for (long count : new long[]{100L,250L}) {
            var s=new ProblemSizeStatistics(0L,Collections.emptySortedMap(),0L,0L,Collections.emptySortedMap(),Math.log10(count));
            System.out.println(count+","+s.approximateProblemScaleLogAsFixedPointLong()+","+MathUtils.getScaledApproximateLog(MathUtils.LOG_PRECISION,10L,count));
        }
    }
}
