// Permutation helpers adapted from Community AbstractTopologicalGraphTest,
// Apache-2.0. See compat/provenance.json. No topology production code is used.
import java.util.*;
public class TopologyPermutationOracle {
    static <T> List<List<T>> permutations(List<T> source, int limit) {
        var out = new ArrayList<List<T>>();
        if (source.isEmpty()) { out.add(new ArrayList<>()); return out; }
        if (limit <= 1) { out.add(new ArrayList<>(source)); return out; }
        var head = source.get(0);
        for (var tail : permutations(source.subList(1, source.size()), limit / source.size())) {
            for (int i=0; i<=tail.size(); i++) {
                var next = new ArrayList<>(tail); next.add(i, head); out.add(next);
            }
        }
        return out;
    }
    public static void main(String[] args) {
        for (int c=0; c<7; c++) {
            int[][] graph = c<2 ? new int[][]{{1,2},{3},{3},{4},{}} :
                c<4 ? new int[][]{{1,2},{3},{3},{4,1},{}} :
                c<6 ? new int[][]{{1,2},{3,2},{4},{5,1},{5,2,3},{}} :
                      new int[][]{{1,2},{2},{4,3},{4,5},{5,1},{}};
            var nodes = new ArrayList<Integer>();
            for(int i=0;i<graph.length;i++) nodes.add(i);
            int count=0;
            long hash=0xcbf29ce484222325L;
            for(var map:permutations(nodes,Integer.MAX_VALUE)) {
                var edges = new ArrayList<List<Integer>>();
                for(int i=0;i<graph.length;i++) for(int j:graph[i]) edges.add(List.of(map.get(i),map.get(j)));
                if(c>=4) Collections.shuffle(edges,new Random(count));
                for(var perm:permutations(edges,c<4?Integer.MAX_VALUE:1000)) {
                    for(var edge:perm) for(int node:edge) hash=(hash^node)*0x100000001b3L;
                    count++;
                }
            }
            System.out.printf("%d %d %016x%n",c,count,hash);
        }
    }
}
