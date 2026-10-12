// Original public metric-name observation against the pinned Community binary.
// No production implementation inspection or copied TEST helper.
import ai.timefold.solver.core.config.solver.monitoring.SolverMetric;
public class SolverMeterNamesProbe {
 public static void main(String[] args) {
  for(var m:SolverMetric.values()) System.out.println(m.name()+"\t"+m.getMeterId());
 }
}
