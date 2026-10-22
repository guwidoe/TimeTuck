// Application benchmark adapted from the RealWorld application spike.
// Compiles against pinned Timefold Community 2.7.0 artifacts; no solver production source copied.
// java ApplicationBenchmark <model.txt> <streams|incremental> <la|sa> <seconds> <seed> <out.txt>
import java.io.*;
import java.nio.file.*;
import java.util.*;
import ai.timefold.solver.core.api.domain.entity.*;
import ai.timefold.solver.core.api.domain.common.PlanningId;
import ai.timefold.solver.core.api.domain.solution.*;
import ai.timefold.solver.core.api.domain.valuerange.ValueRangeProvider;
import ai.timefold.solver.core.api.domain.variable.PlanningVariable;
import ai.timefold.solver.core.api.score.HardSoftScore;
import ai.timefold.solver.core.api.score.calculator.IncrementalScoreCalculator;
import ai.timefold.solver.core.api.score.stream.*;
import ai.timefold.solver.core.api.solver.SolverFactory;
import ai.timefold.solver.core.config.heuristic.selector.move.composite.UnionMoveSelectorConfig;
import ai.timefold.solver.core.config.heuristic.selector.move.generic.ChangeMoveSelectorConfig;
import ai.timefold.solver.core.config.heuristic.selector.move.generic.SwapMoveSelectorConfig;
import ai.timefold.solver.core.config.localsearch.LocalSearchPhaseConfig;
import ai.timefold.solver.core.config.localsearch.decider.acceptor.LocalSearchAcceptorConfig;
import ai.timefold.solver.core.config.localsearch.decider.forager.LocalSearchForagerConfig;
import ai.timefold.solver.core.config.score.director.ScoreDirectorFactoryConfig;
import ai.timefold.solver.core.config.solver.EnvironmentMode;
import ai.timefold.solver.core.config.solver.SolverConfig;
import ai.timefold.solver.core.config.solver.termination.TerminationConfig;
import ai.timefold.solver.core.impl.heuristic.selector.common.decorator.SelectionFilter;
import ai.timefold.solver.core.impl.heuristic.selector.move.generic.SelectorBasedChangeMove;
import ai.timefold.solver.core.impl.heuristic.selector.move.generic.SelectorBasedSwapMove;
import ai.timefold.solver.core.impl.score.director.ScoreDirector;
import ai.timefold.solver.core.impl.solver.DefaultSolver;
import static ai.timefold.solver.core.api.score.stream.Joiners.*;

public final class ApplicationBenchmark {
    // Static model data (single solve per process).
    static int P, G, S, UC, REP_MAX, REP_SQ, REP_W;
    static int[][] CAP;
    static Req[] REQS;
    static int[][] REQS_BY_PERSON;
    static Bal[] BALS;
    static Slot[] SLOTS;

    @PlanningEntity public static final class Seat {
        @PlanningId public int id;
        public int p, s;
        @PlanningPin public boolean pinned;
        @PlanningVariable(valueRangeProviderRefs = "groups") public Integer group;
        public Seat() {}
        Seat(int id, int p, int s, int g, boolean pinned) { this.id = id; this.p = p; this.s = s; this.group = g; this.pinned = pinned; }
    }
    public record Req(int id, int kind, long w, int p, int q, long sessions) {
        boolean hard() { return kind < 2; }
        boolean wantsTogether() { return kind == 0 || kind == 2; }
        boolean inSession(int s) { return (sessions >> s & 1) != 0; }
    }
    public record Bal(long w, boolean atLeast, long sessions, long groups, int[] targets, int[] valueOf) {}
    public record Slot(int id, int b, int ti) {}

    @PlanningSolution public static final class Plan {
        @ValueRangeProvider(id = "groups") @ProblemFactCollectionProperty public List<Integer> groups;
        @PlanningEntityCollectionProperty public List<Seat> seats;
        @ProblemFactCollectionProperty public List<Req> reqs;
        @ProblemFactCollectionProperty public List<Slot> slots;
        @PlanningScore public HardSoftScore score;
    }

    static long repeatCost(long count) {
        long e = Math.max(0, count - REP_MAX);
        return REP_W * (REP_SQ == 1 ? e * e : e);
    }
    static long balanceGap(Bal b, int ti, long hits) {
        long t = b.targets[ti];
        return b.atLeast ? Math.max(0, t - hits) : Math.abs(t - hits);
    }

    public static final class Streams implements ConstraintProvider {
        @Override public Constraint[] defineConstraints(ConstraintFactory f) {
            return new Constraint[] {
                f.forEach(Seat.class)
                    .groupBy(a -> a.s * G + a.group, ConstraintCollectors.count())
                    .filter((k, c) -> c > CAP[k / G][k % G])
                    .penalize(HardSoftScore.ONE_HARD, (k, c) -> c - CAP[k / G][k % G])
                    .asConstraint("capacity"),
                f.forEachUniquePair(Seat.class, equal(a -> a.s), equal(a -> a.group))
                    .groupBy((a, b) -> Math.min(a.p, b.p) * P + Math.max(a.p, b.p), ConstraintCollectors.countBi())
                    .impact(HardSoftScore.ONE_SOFT, (k, c) -> UC - repeatCost(c))
                    .asConstraint("contacts"),
                f.forEach(Req.class)
                    .join(Seat.class, equal(Req::p, a -> a.p), filtering((r, a) -> r.inSession(a.s)))
                    .join(Seat.class, equal((r, a) -> r.q(), b -> b.p), equal((r, a) -> a.s, b -> b.s))
                    .filter((r, a, b) -> r.hard() && (a.group.equals(b.group) != r.wantsTogether()))
                    .penalize(HardSoftScore.ONE_HARD).asConstraint("hard pairs"),
                f.forEach(Req.class)
                    .join(Seat.class, equal(Req::p, a -> a.p), filtering((r, a) -> r.inSession(a.s)))
                    .join(Seat.class, equal((r, a) -> r.q(), b -> b.p), equal((r, a) -> a.s, b -> b.s))
                    .filter((r, a, b) -> !r.hard() && (a.group.equals(b.group) != r.wantsTogether()))
                    .penalize(HardSoftScore.ONE_SOFT, (r, a, b) -> r.w()).asConstraint("soft pairs"),
                f.forEach(Seat.class)
                    .join(Slot.class, filtering((a, sl) -> {
                        Bal b = BALS[sl.b()];
                        return (b.sessions() >> a.s & 1) != 0 && (b.groups() >> a.group & 1) != 0;
                    }))
                    .groupBy((a, sl) -> ((long) sl.id() * S + a.s) * G + a.group,
                        ConstraintCollectors.sum((a, sl) -> BALS[sl.b()].valueOf()[a.p] == sl.ti() ? 1L : 0L))
                    .penalize(HardSoftScore.ONE_SOFT, (k, hits) -> {
                        Slot sl = SLOTS[(int) (k / G / S)];
                        Bal b = BALS[sl.b()];
                        return b.w() * balanceGap(b, sl.ti(), hits);
                    }).asConstraint("attribute balance"),
            };
        }
    }

    /** Hand-written delta scoring with domain caches (occupancy, members, pair counts). */
    public static final class Incremental implements IncrementalScoreCalculator<Plan, HardSoftScore> {
        int[][] occ; int[][][] members; int[][] size; int[][] pairCount; Integer[][] loc;
        long[][][][] balHits; // [b][s][g][ti]
        long hard, soft;

        @Override public void resetWorkingSolution(Plan plan) {
            occ = new int[S][G]; members = new int[S][G][P]; size = new int[S][G];
            pairCount = new int[P][P]; loc = new Integer[P][S];
            balHits = new long[BALS.length][S][G][];
            for (int b = 0; b < BALS.length; b++) for (int s = 0; s < S; s++) for (int g = 0; g < G; g++)
                balHits[b][s][g] = new long[BALS[b].targets().length];
            hard = 0; soft = 0;
            for (Seat a : plan.seats) insert(a);
        }
        long capCost(int s, int g) { return Math.max(0, occ[s][g] - CAP[s][g]); }
        long balCost(int s, int g) {
            if (occ[s][g] == 0) return 0; // streams only see nonempty groups
            long c = 0;
            for (int b = 0; b < BALS.length; b++) {
                Bal bal = BALS[b];
                if ((bal.sessions() >> s & 1) == 0 || (bal.groups() >> g & 1) == 0) continue;
                for (int ti = 0; ti < bal.targets().length; ti++) c += bal.w() * balanceGap(bal, ti, balHits[b][s][g][ti]);
            }
            return c;
        }
        long pairValue(int count) { return count == 0 ? 0 : UC - repeatCost(count); }
        long reqCost(Seat a, int sign) {
            long[] hs = new long[2];
            for (int r : REQS_BY_PERSON[a.p]) {
                Req req = REQS[r];
                if (!req.inSession(a.s)) continue;
                int other = req.p() == a.p ? req.q() : req.p();
                Integer og = loc[other][a.s];
                if (og == null || loc[a.p][a.s] == null) continue;
                if (og.equals(loc[a.p][a.s]) != req.wantsTogether()) {
                    if (req.hard()) hs[0]++; else hs[1] += req.w();
                }
            }
            hard -= sign * hs[0]; soft -= sign * hs[1];
            return 0;
        }
        void insert(Seat a) {
            if (a.group == null) return;
            int s = a.s, g = a.group, p = a.p;
            hard += capCost(s, g); soft += balCost(s, g);
            for (int i = 0; i < size[s][g]; i++) {
                int q = members[s][g][i], lo = Math.min(p, q), hi = Math.max(p, q);
                soft -= pairValue(pairCount[lo][hi]);
                soft += pairValue(++pairCount[lo][hi]);
            }
            members[s][g][size[s][g]++] = p; occ[s][g]++;
            for (int b = 0; b < BALS.length; b++) { int v = BALS[b].valueOf()[p]; if (v >= 0) balHits[b][s][g][v]++; }
            hard -= capCost(s, g); soft -= balCost(s, g);
            loc[p][s] = g;
            reqCost(a, 1);
        }
        void retract(Seat a) {
            if (a.group == null) return;
            int s = a.s, g = a.group, p = a.p;
            reqCost(a, -1);
            loc[p][s] = null;
            hard += capCost(s, g); soft += balCost(s, g);
            int at = 0;
            while (members[s][g][at] != p) at++;
            members[s][g][at] = members[s][g][--size[s][g]]; occ[s][g]--;
            for (int b = 0; b < BALS.length; b++) { int v = BALS[b].valueOf()[p]; if (v >= 0) balHits[b][s][g][v]--; }
            for (int i = 0; i < size[s][g]; i++) {
                int q = members[s][g][i], lo = Math.min(p, q), hi = Math.max(p, q);
                soft -= pairValue(pairCount[lo][hi]);
                soft += pairValue(--pairCount[lo][hi]);
            }
            hard -= capCost(s, g); soft -= balCost(s, g);
        }
        @Override public void beforeVariableChanged(Object entity, String variable) { retract((Seat) entity); }
        @Override public void afterVariableChanged(Object entity, String variable) { insert((Seat) entity); }
        @Override public HardSoftScore calculateScore() { return HardSoftScore.of(hard, soft); }
    }

    /** Swaps only within one session, between different groups. */
    public static final class SameSessionSwap implements SelectionFilter<Plan, SelectorBasedSwapMove<Plan>> {
        @Override public boolean accept(ScoreDirector<Plan> d, SelectorBasedSwapMove<Plan> m) {
            Seat a = (Seat) m.getLeftEntity(), b = (Seat) m.getRightEntity();
            return a.s == b.s && !Objects.equals(a.group, b.group);
        }
    }
    /** Changes only into a different group with a spare seat in that session. */
    public static final class SpareSeatChange implements SelectionFilter<Plan, SelectorBasedChangeMove<Plan>> {
        // The solver works on a planning clone, so occupancy must come from the working solution's
        // seats; the input seats keep their initial groups and would admit moves into full groups.
        private Plan indexed;
        private Seat[][] seatsBySession;

        @Override public boolean accept(ScoreDirector<Plan> d, SelectorBasedChangeMove<Plan> m) {
            Seat a = (Seat) m.getEntity();
            Integer to = (Integer) m.getToPlanningValue();
            if (to == null || to.equals(a.group)) return false;
            int n = 0;
            for (Seat o : workingSessionSeats(d.getWorkingSolution())[a.s]) if (to.equals(o.group)) n++;
            return n < CAP[a.s][to];
        }

        private Seat[][] workingSessionSeats(Plan working) {
            if (working != indexed) {
                List<List<Seat>> bySession = new ArrayList<>();
                for (int s = 0; s < S; s++) bySession.add(new ArrayList<>());
                for (Seat seat : working.seats) bySession.get(seat.s).add(seat);
                seatsBySession = bySession.stream().map(l -> l.toArray(Seat[]::new)).toArray(Seat[][]::new);
                indexed = working;
            }
            return seatsBySession;
        }
    }

    public static void main(String[] args) throws Exception {
        var in = new Scanner(Files.newBufferedReader(Path.of(args[0])));
        P = in.nextInt(); G = in.nextInt(); S = in.nextInt();
        CAP = new int[S][G];
        for (int s = 0; s < S; s++) for (int g = 0; g < G; g++) CAP[s][g] = in.nextInt();
        UC = in.nextInt(); REP_MAX = in.nextInt(); REP_SQ = in.nextInt(); REP_W = in.nextInt();
        var plan = new Plan();
        plan.groups = new ArrayList<>(); for (int g = 0; g < G; g++) plan.groups.add(g);
        plan.seats = new ArrayList<>();
        int nv = in.nextInt();
        for (int v = 0; v < nv; v++) plan.seats.add(new Seat(v, in.nextInt(), in.nextInt(), in.nextInt(), in.nextInt() == 1));
        int nr = in.nextInt();
        REQS = new Req[nr];
        List<List<Integer>> byPerson = new ArrayList<>(); for (int p = 0; p < P; p++) byPerson.add(new ArrayList<>());
        for (int r = 0; r < nr; r++) {
            in.nextInt();
            REQS[r] = new Req(r, in.nextInt(), in.nextLong(), in.nextInt(), in.nextInt(), in.nextLong());
            byPerson.get(REQS[r].p()).add(r); byPerson.get(REQS[r].q()).add(r);
        }
        REQS_BY_PERSON = byPerson.stream().map(l -> l.stream().mapToInt(Integer::intValue).toArray()).toArray(int[][]::new);
        int nb = in.nextInt();
        BALS = new Bal[nb];
        List<Slot> slots = new ArrayList<>();
        for (int b = 0; b < nb; b++) {
            long w = in.nextLong(); boolean al = in.nextInt() == 1; long sm = in.nextLong(); long gm = in.nextLong();
            int[] targets = new int[in.nextInt()];
            for (int t = 0; t < targets.length; t++) { targets[t] = in.nextInt(); slots.add(new Slot(slots.size(), b, t)); }
            int[] valueOf = new int[P]; Arrays.fill(valueOf, -1);
            int nh = in.nextInt();
            for (int h = 0; h < nh; h++) { int p = in.nextInt(); valueOf[p] = in.nextInt(); }
            BALS[b] = new Bal(w, al, sm, gm, targets, valueOf);
        }
        SLOTS = slots.toArray(Slot[]::new);
        plan.reqs = Arrays.asList(REQS); plan.slots = slots;

        String mode = args[1], acceptance = args[2];
        double seconds = Double.parseDouble(args[3]);
        long seed = Long.parseLong(args[4]);
        var acceptor = acceptance.equals("la")
            ? new LocalSearchAcceptorConfig().withLateAcceptanceSize(400)
            : new LocalSearchAcceptorConfig().withSimulatedAnnealingStartingTemperature("0hard/" + args[6] + "soft");
        var phase = new LocalSearchPhaseConfig()
            .withMoveSelectorConfig(new UnionMoveSelectorConfig().withMoveSelectors(
                new ChangeMoveSelectorConfig().withFilterClass(SpareSeatChange.class),
                new SwapMoveSelectorConfig().withFilterClass(SameSessionSwap.class)))
            .withAcceptorConfig(acceptor)
            .withForagerConfig(new LocalSearchForagerConfig().withAcceptedCountLimit(1));
        var config = new SolverConfig().withSolutionClass(Plan.class).withEntityClasses(Seat.class)
            .withEnvironmentMode(EnvironmentMode.NO_ASSERT).withRandomSeed(seed)
            .withPhases(phase)
            .withTerminationConfig(new TerminationConfig().withMillisecondsSpentLimit((long) (seconds * 1000)));
        config = mode.equals("streams")
            ? config.withConstraintProviderClass(Streams.class)
            : config.withScoreDirectorFactory(new ScoreDirectorFactoryConfig().withIncrementalScoreCalculatorClass(Incremental.class));
        var factory = SolverFactory.<Plan>create(config);
        HardSoftScore initialScore = ai.timefold.solver.core.api.solver.SolutionManager.<Plan, HardSoftScore>create(factory).update(plan);
        var solver = (DefaultSolver<Plan>) factory.buildSolver();
        long started = System.nanoTime();
        Plan best = solver.solve(plan);
        long elapsedNs = System.nanoTime() - started;
        double elapsed = elapsedNs / 1e9;
        System.out.printf(java.util.Locale.ROOT, "{\"elapsed_ns\":%d,\"evaluations\":%d,\"initial_score\":[%d,%d],\"best_score\":[%d,%d],\"mode\":\"%s\"}%n",
            elapsedNs, solver.getMoveEvaluationCount(), initialScore.hardScore(), initialScore.softScore(), best.score.hardScore(), best.score.softScore(), mode);
        System.err.printf("timefold %s/%s: %.2fs, %d move evaluations (%.0f/s), score %dhard/%.2fsoft%n",
            mode, acceptance, elapsed, solver.getMoveEvaluationCount(), solver.getMoveEvaluationCount() / elapsed,
            best.score.hardScore(), best.score.softScore() / 100.0);
        try (var out = new PrintWriter(Files.newBufferedWriter(Path.of(args[5])))) {
            for (Seat a : best.seats) out.println(a.p + " " + a.s + " " + a.group);
        }
    }
}
