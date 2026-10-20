"use strict";
const $ = (id) => document.getElementById(id);
const states = ["passed", "failed", "pending", "excluded", "unverified"];
const names = {
    passed: "Passed",
    failed: "Failed",
    pending: "Pending",
    excluded: "Excluded",
    unverified: "Unverified",
};
const format = new Intl.NumberFormat("en");
let report,
    selected,
    limit = 40,
    loading = false,
    lastStamp,
    fresh = true,
    completionFresh = true,
    completionLive = false,
    nativeFresh = false;
const n = (value) => format.format(value);
const pct = (value) => (value == null ? "—" : `${value.toFixed(2)}%`);
// Derive display precision from counts, avoiding double rounding of report.percent.
const coveragePct = ({ passed, total }) => pct(total ? (100 * passed) / total : 0);
function node(tag, cls, text) {
    const e = document.createElement(tag);
    if (cls) e.className = cls;
    if (text != null) e.textContent = text;
    return e;
}
function timestamp(value) {
    return new Date(value).toLocaleString(undefined, {
        dateStyle: "medium",
        timeStyle: "short",
    });
}
function segments(target, data) {
    target.replaceChildren();
    states.forEach((state) => {
        if (!data[state]) return;
        const e = node("span", `segment ${state}`);
        e.style.width = `${(100 * data[state]) / data.total}%`;
        e.title = `${names[state]}: ${n(data[state])}`;
        target.append(e);
    });
}
function render() {
    const focused = trackedFocus();
    const s = report.summary;
    $("reference").textContent =
        `Rust / Timefold Community ${report.reference.tag.replace(/^v/, "")}`;
    $("upstream-link").href =
        `${report.reference.repository.replace(/\.git$/, "")}/tree/${report.reference.commit}`;
    $("overall-percent").textContent = coveragePct(s);
    $("overall-count").textContent = `${n(s.passed)} / ${n(s.total)} methods`;
    $("overall-bar").setAttribute(
        "aria-label",
        `${s.passed} passing adaptations of ${s.total} discovered upstream test methods`,
    );
    segments($("overall-bar"), s);
    $("legend").replaceChildren(
        ...states.map((state) => {
            const e = node("span");
            e.append(
                node("i", `dot ${state}`),
                node("b", "", n(s[state])),
                document.createTextNode(names[state]),
            );
            return e;
        }),
    );
    $("families").replaceChildren(
        ...report.families.map((f) => {
            const b = node("button", "family-row");
            b.type = "button";
            b.dataset.family = f.name;
            b.setAttribute("aria-controls", "selected-detail");
            b.setAttribute("aria-pressed", String(f.name === selected));
            b.setAttribute(
                "aria-label",
                `${f.name}: ${f.passed} of ${f.total} passed, ${coveragePct(f)}`,
            );
            const count = node("span", "family-count");
            count.append(
                node("strong", "", n(f.passed)),
                document.createTextNode(` / ${n(f.total)}`),
            );
            const track = node("span", "family-track");
            segments(track, f);
            b.append(
                node("span", "family-name", f.name),
                count,
                node(
                    "span",
                    `family-percent${f.passed ? "" : " empty"}`,
                    coveragePct(f),
                ),
                track,
            );
            b.addEventListener("click", () => {
                selected = f.name;
                limit = 40;
                $("test-search").value = "";
                $("test-status").value = "all";
                render();
                if (matchMedia("(max-width:760px)").matches)
                    $("selected-detail").scrollIntoView();
            });
            return b;
        }),
    );
    renderCompletion();
    renderNativeReadiness();
    renderDetail();
    renderBenchmarks();
    renderBenchmarks("solver");
    renderBenchmarks("mixed");
    renderBenchmarks("construction");
    renderBenchmarks("repair");
    renderBenchmarks("list_repair");
    renderBenchmarks("heuristic");
    renderBenchmarks("list");
    renderBenchmarks("kopt");
    renderBenchmarks("stream");
    renderBenchmarks("pillar");
    renderBenchmarks("concurrent");
    renderBenchmarks("aggregate");
    renderBenchmarks("sequence");
    renderDifferential();
    $("inventory-note").textContent =
        `${n(s.total)} annotated methods in ${n(report.inventory.source_files)} Java test-source files at ${report.reference.commit.slice(0, 12)}. ${report.inventory.limitations}`;
    $("updated").textContent =
        `Report updated ${timestamp(report.generated_at)} · reloads every 15s`;
    $("content").hidden = false;
    restoreFocus(focused);
}
function renderNativeReadiness() {
    const model = report.native_readiness;
    const summary = $("native-summary"), blockers = $("native-blockers"), evidenceBlockers = $("native-evidence-blockers");
    const requirements = $("native-requirements"), boundaries = $("native-boundaries");
    for (const target of [summary, blockers, evidenceBlockers, requirements, boundaries]) target.replaceChildren();
    const available = model?.schema_version === 1 && model.target_set === "original-native-goal-v1" && typeof model.model_digest === "string" && Array.isArray(model.requirements) && model.requirements.length > 0 && Array.isArray(model.checklist) && model.checklist.length > 0;
    $("native-evidence").hidden = !available;
    if (!available) {
        $("native-status").textContent = "Evidence unavailable";
        $("native-freshness").textContent = "Regenerate the report to assess native readiness. Missing evidence cannot establish completion.";
        return;
    }
    const current = fresh && nativeFresh && completionFresh && completionLive;
    const ready = model.native_ready === true && current;
    $("native-status").textContent = ready ? "Tracked native targets met" : current ? "Targets remain" : "Verification needed";
    $("native-status").className = ready ? "completion-met" : "completion-open";
    const capabilities = model.requirements.filter(row => row.status === "met").length;
    const performance = Array.isArray(model.performance) ? model.performance : [];
    const met = performance.reduce((sum, row) => sum + (Number.isInteger(row.met) ? row.met : 0), 0);
    const total = performance.reduce((sum, row) => sum + (Number.isInteger(row.total) ? row.total : 0), 0);
    for (const [label, value, complete] of [
        ["Requirements with current evidence", `${capabilities} / ${model.requirements.length}`, capabilities === model.requirements.length],
        [performance.every(row => row.freshness === "current") ? "Timed workloads at Java parity" : "Last measured workloads at Java parity", total ? `${met} / ${total}` : "Unmeasured", total > 0 && met === total],
        ["Comparison reports current", `${report.completion?.current_jobs ?? 0} / ${report.completion?.total_jobs ?? 0}`, report.completion?.current_jobs > 0 && report.completion.current_jobs === report.completion.total_jobs],
    ]) {
        const item = node("div", "native-metric");
        item.append(node("strong", complete ? "completion-met" : "completion-open", value), node("span", "", label));
        summary.append(item);
    }
    const open = Array.isArray(model.blockers) ? model.blockers : [];
    for (const text of open) evidenceBlockers.append(node("li", "", `Needed: ${text}`));
    const jobsCurrent = report.completion?.current_jobs ?? 0, jobsTotal = report.completion?.total_jobs ?? 0;
    if (jobsTotal > jobsCurrent) blockers.append(node("li", "", `${jobsTotal - jobsCurrent} comparison reports need measurements for the current source.`));
    for (const row of performance.filter(item => item.status === "not_met")) {
        const worst = row.bottleneck;
        if (worst?.name) blockers.append(node("li", "", `${worst.name}: performance target unmet${row.freshness === "current" ? "" : " in previous evidence"}. ${worst.detail || ""}`));
    }
    if (!current) blockers.prepend(node("li", "", "Live verification or target freshness is missing. Readiness cannot be established."));
    if (!ready && !blockers.children.length) blockers.append(node("li", "", "Capability or contract evidence still needs verification. Open the evidence details below."));
    for (const row of model.requirements) {
        const item = node("li");
        item.append(node("strong", "", row.label || row.id), node("span", row.status === "met" ? "completion-met" : "completion-open", row.status === "met" ? "Evidenced" : "Evidence needed"), node("p", "", row.limits || row.contract || ""));
        for (const error of row.evidence_errors || []) item.append(node("small", "", error));
        requirements.append(item);
    }
    for (const row of model.boundaries || []) {
        const item = node("li");
        item.append(node("strong", "", ({"connected-range-history": "Consistent connected ranges", "list-planning-value-metadata": "Stable list value capture", "cyclic-supplier-reference-errors": "Cyclic suppliers with unavailable Java outcomes"})[row.id] || row.id), node("span", "", row.reason || row.disposition || ""));
        const counts = row.counts || {};
        if (Object.keys(counts).length) item.append(node("small", "", Object.entries(counts).map(([key, value]) => `${key.replaceAll("_", " ")}: ${typeof value === "number" ? n(value) : value}`).join(" · ")));
        if (row.status !== "resolved") item.append(node("small", "completion-open", "Boundary evidence unresolved"));
        boundaries.append(item);
    }
    const methodCount = Array.isArray(model.method_boundaries) ? model.method_boundaries.length : 0;
    if (methodCount) boundaries.append(node("li", "", `${methodCount} pending original methods retain their JVM or private-representation scope boundaries. Their inventory status is unchanged.`));
    $("native-freshness").textContent = current
        ? "Readiness uses current source, targets and comparison evidence. These bounded requirements do not estimate remaining engineering effort or prove universal equivalence."
        : "Last saved evidence. Source verification, policy freshness and live status must be current before readiness can be established.";
}
function renderCompletion() {
    const model = report.completion;
    const track = $("completion-track"), rows = $("completion-components");
    const breakdown = $("completion-breakdown"), blockers = $("completion-blockers");
    track.replaceChildren(); rows.replaceChildren(); breakdown.replaceChildren(); blockers.replaceChildren();
    if (!model || !Array.isArray(model.components)) {
        $("completion-percent").textContent = "—";
        $("completion-status").textContent = "Completion data is not available in this report.";
        $("completion-freshness").textContent = "Regenerate the report to calculate progress. Missing evidence is never treated as complete.";
        $("completion-calculation").hidden = true;
        track.setAttribute("aria-label", "Completion data unavailable");
        return;
    }
    $("completion-calculation").hidden = false;
    const percent = Number.isFinite(model.index_percent) ? model.index_percent : 0;
    const ready = model.ready && fresh && completionFresh && completionLive;
    $("completion-percent").textContent = `${(ready ? 100 : Math.min(percent, 99.9)).toFixed(1)}%`;
    $("completion-status").textContent = ready ? "All tracked targets met with current evidence" : "Targets remain before completion";
    if (!fresh || !completionFresh || !completionLive) $("completion-status").textContent = "Last measured progress · verification needed";
    track.setAttribute("aria-label", `Weighted completion index ${percent.toFixed(1)} percent. Not an estimate of remaining effort.`);
    const labels = {features: "Feature coverage", behavior: "Behavioral agreement", performance: model.target.java_time_multiplier === 1 ? "Java performance parity" : "Java performance target", evidence: "Current verification"};
    const descriptions = {
        features: "Passing adaptations within scope, balanced by feature area.",
        behavior: "Weakest observed agreement in each comparison family.",
        performance: "Weakest workload in each family, with equal or better quality.",
        evidence: "Current comparison reports; successful Rust verification required.",
    };
    const jobLabels = {
        differential: "Score arithmetic", solver_differential: "Basic & mixed solving",
        list_differential: "List moves & metadata", stream_differential: "Constraint stream behavior",
        performance: "Score arithmetic", solver_performance: "Exact search",
        mixed_performance: "Mixed planning", heuristic_performance: "Heuristic search",
        construction_performance: "Construction", repair_performance: "Basic repair",
        list_repair_performance: "List repair", list_performance: "List planning",
        kopt_performance: "K-opt", stream_performance: "Constraint streams",
        concurrent_performance: "Concurrent shadows", sequence_performance: "Sequence collectors",
        aggregate_performance: "Aggregate collectors", pillar_performance: "Pillar moves",
    };
    const formula = [];
    for (const c of model.components) {
        const value = Number.isFinite(c.percent) ? Math.max(0, Math.min(100, c.percent)) : 0;
        const label = labels[c.id] || c.label;
        const slot = node("span", `completion-slot completion-${c.id}`);
        slot.style.flex = String(c.weight);
        const fill = node("span", "completion-fill"); fill.style.width = `${value}%`; slot.append(fill);
        slot.title = `${label}: ${value.toFixed(1)}%, ${c.weight}% of the index`;
        track.append(slot);
        const row = node("div", `completion-row completion-${c.id}`);
        const title = node("div", "completion-label"); title.append(node("strong", "", label), node("span", "", c.id === "performance" ? `Rust ≤ ${model.target.java_time_multiplier}× Java time, with equal or better quality.` : descriptions[c.id] || c.detail));
        const meter = node("div", "completion-meter");
        meter.setAttribute("role", "progressbar"); meter.setAttribute("aria-label", label);
        meter.setAttribute("aria-valuemin", "0"); meter.setAttribute("aria-valuemax", "100"); meter.setAttribute("aria-valuenow", String(value));
        const bar = node("span"); bar.style.width = `${value}%`; meter.append(bar);
        const number = node("div", "completion-row-value"); number.append(node("strong", "", `${value.toFixed(1)}%`), node("span", "", `${c.weight}% weight`));
        row.append(title, meter, number); rows.append(row);
        formula.push(`${label} ${c.weight}%`);
        const group = node("section", "completion-group"); group.append(node("h3", "", label));
        group.append(node("p", "", c.detail));
        const list = node("ul");
        for (const item of c.items || []) {
            const entry = node("li");
            const name = node("span", "", jobLabels[item.name] || item.name);
            const status = item.status === "met" ? "Target met" : item.status === "unknown" ? "Evidence missing" : "Below target";
            const last = item.freshness === "stale" ? " · previous evidence" : item.freshness === "unknown" ? " · freshness unknown" : "";
            const amount = Number.isFinite(item.percent) ? `${item.percent.toFixed(1)}%` : "—";
            entry.append(name, node("span", item.status === "met" ? "completion-met" : "completion-open", `${amount} · ${status}${last}`));
            if (item.detail) entry.append(node("small", "", item.detail));
            list.append(entry);
        }
        group.append(list); breakdown.append(group);
    }
    $("completion-formula").textContent = `Index weights: ${formula.join(" · ")}. The weights are explicit project settings, not estimates of task duration.`;
    $("completion-target").textContent = `Performance target: Rust median time ≤ ${model.target.java_time_multiplier}× Java, with the same or better solution quality and successful workload checks. Comparisons measure the configured solves, not identical execution work.`;
    $("completion-freshness").textContent = `${model.current_jobs} / ${model.total_jobs} comparison reports current${completionLive ? "" : " · live status unavailable"} · index includes previous evidence · report ${timestamp(report.generated_at)}`;
    for (const text of model.blockers || []) blockers.append(node("li", "", text));
    if (!fresh) blockers.prepend(node("li", "", "Source changes need verification before the last measured progress can be considered current."));
    if (!completionFresh) blockers.prepend(node("li", "", "The saved index uses an older completion model or target."));
}

function trackedFocus() {
    return {
        family: document.activeElement?.dataset.family,
        method: document.activeElement?.dataset.method,
    };
}
function restoreFocus(focused) {
    if (focused.family) {
        const target = [...document.querySelectorAll(".family-row")].find(
            (element) => element.dataset.family === focused.family,
        );
        (target || $("test-search")).focus({ preventScroll: true });
    } else if (focused.method) {
        const target = [...document.querySelectorAll(".test-class a")].find(
            (element) => element.dataset.method === focused.method,
        );
        (target || $("test-search")).focus({ preventScroll: true });
    }
}
function renderDetail() {
    const family = report.families.find((f) => f.name === selected);
    const methods = report.methods.filter((m) => m.family === selected);
    $("detail-title").textContent = selected;
    $("detail-count").textContent =
        `${n(family.passed)} / ${n(family.total)} passed`;
    $("detail-map").setAttribute(
        "aria-label",
        `${selected}: ` + states.map((s) => `${family[s]} ${s}`).join(", "),
    );
    $("detail-map").replaceChildren(
        ...methods.map((m) => {
            const e = node("span", `test-cell ${m.status}`);
            e.title = `${m.path.split("/").pop()}: ${m.method} — ${names[m.status]}`;
            return e;
        }),
    );
    renderTests();
}
function renderTests() {
    const focused = trackedFocus();
    const query = $("test-search").value.toLowerCase(),
        status = $("test-status").value;
    const methods = report.methods.filter(
        (m) =>
            m.family === selected &&
            (status === "all" || m.status === status) &&
            `${m.method} ${m.path}`.toLowerCase().includes(query),
    );
    // Surface implemented cases first; preserve source order within each result group.
    methods.sort((a, b) => states.indexOf(a.status) - states.indexOf(b.status));
    $("test-list").replaceChildren(
        ...methods.slice(0, limit).map((m) => {
            const item = node("div", "test-item"),
                line = node("div", "test-line"),
                context = node("div", "test-class");
            line.append(
                node("span", "test-name", m.method),
                node("span", `test-status ${m.status}`, names[m.status]),
            );
            const link = node(
                "a",
                "",
                m.path
                    .split("/")
                    .pop()
                    .replace(/\.java$/, ""),
            );
            link.dataset.method = m.id;
            link.href = `${report.reference.repository.replace(/\.git$/, "")}/blob/${report.reference.commit}/${m.path}#L${m.line}`;
            link.target = "_blank";
            link.rel = "noopener noreferrer";
            context.append(link);
            item.append(line, context);
            if (
                m.status === "excluded" ||
                m.status === "failed" ||
                m.status === "unverified"
            )
                item.append(node("p", "test-reason", m.reason));
            return item;
        }),
    );
    if (!methods.length)
        $("test-list").append(
            node(
                "p",
                "empty-results",
                "No tests match these filters. Try another name or result.",
            ),
        );
    $("list-count").textContent =
        `Showing ${n(Math.min(limit, methods.length))} of ${n(methods.length)} methods`;
    $("show-more").hidden = limit >= methods.length;
    restoreFocus(focused);
}
function renderBenchmarks(kind = "score") {
    const solver = kind !== "score";
    const heuristic = kind === "heuristic" || kind === "list" || kind === "kopt" || kind === "stream" || kind === "construction" || kind === "repair" || kind === "list_repair" || kind === "mixed";
    const prefix = solver ? `${kind}-` : "";
    const target = (id) => document.getElementById(prefix + id);
    const unit = solver ? "ms" : "ns";
    const precision = (kind === "pillar" || kind === "concurrent" || kind === "sequence" || kind === "aggregate") ? 5 : solver ? 3 : 2;
    const p = (solver ? report[`${kind}_performance`] : report.performance) || {},
        box = target("benchmarks");
    box.replaceChildren();
    const missing = !p.workloads?.length;
    if (kind === "heuristic") {
        $("heuristic-algorithm-count").textContent = missing
            ? "Heuristic configurations"
            : `${new Set(p.workloads.map(w => w.algorithm)).size} heuristic configurations`;
    }
    target("benchmark-state").hidden = !missing && p.status === "current" && fresh;
    target("benchmark-state").textContent = missing
        ? "No performance comparison recorded yet. Run the matched benchmark to measure both runtimes."
        : p.status === "stale" || !fresh
          ? "Earlier measurement — source files have changed. Rerun the benchmark before drawing conclusions."
          : "";
    const label = {
        simple: "Simple score",
        hard_soft: "Hard / soft score",
        hard_medium_soft: "Hard / medium / soft score",
    };
    (p.workloads || []).forEach((w) => {
        const b = node("article", "benchmark"),
            ratio = node("div", "benchmark-ratio");
        ratio.append(
            document.createTextNode(`${w.java_over_rust.toFixed(2)}×`),
            node("span", "", heuristic && !w.equal_scores ? "time ratio · quality differs" : "median ratio"),
        );
        b.append(ratio, node("h3", "", label[w.name] || w.name));
        const maximum = Math.max(w.rust.median, w.java.median);
        ["rust", "java"].forEach((runtime) => {
            const row = node("div", "timing"),
                track = node("div"),
                bar = node("div", `bar ${runtime}`);
            bar.style.width = `${(100 * w[runtime].median) / maximum}%`;
            track.append(bar);
            row.append(
                node(
                    "span",
                    "timing-label",
                    runtime === "rust" ? "Rust" : "Java",
                ),
                track,
                node("b", "", `${w[runtime].median.toFixed(precision)} ${unit}`),
            );
            b.append(row);
        });
        b.append(
            node(
                "p",
                "range",
                `Observed ranges · Rust ${w.rust.min.toFixed(precision)}–${w.rust.max.toFixed(precision)} ${unit} · Java ${w.java.min.toFixed(precision)}–${w.java.max.toFixed(precision)} ${unit}`,
            ),
        );
        if (heuristic) {
            const scoreText = (runtime) => `${w[runtime].score[0]}hard/${w[runtime].score[1]}soft`;
            const feasible = (runtime) => w[runtime].score[0] >= 0;
            b.append(node("p", "range", `Solution quality · Rust ${scoreText("rust")} · Java ${scoreText("java")}`));
            const optimum = Array.isArray(w.optimal_score)
                ? `Known optimum: ${w.optimal_score[0]}hard/${w.optimal_score[1]}soft.`
                : "Optimum unknown for this workload.";
            b.append(node("p", "range", `${w.equal_scores ? "Equal final scores" : "Different final scores"}. Rust ${feasible("rust") ? "feasible" : "infeasible"}; Java ${feasible("java") ? "feasible" : "infeasible"}. ${optimum}`));
            if (kind === "mixed") {
                const cold = w.start_mode === "cold";
                b.append(node("p", "range", cold
                    ? "Cold start · missing depot choices and customer placements are constructed before local search; existing pins remain."
                    : "Warm start · local search begins from a complete feasible assignment."));
                if (w.native_list_registry_policy) {
                    b.append(node("p", "range", w.native_list_registry_policy === "ordinal"
                        ? "List registry · checked ordinals. Basic registries remain Eq-only."
                        : "List registry · Eq-only."));
                    const controls = w.eq_only_registry_controls;
                    if (controls?.samples?.length) {
                        const times = [...controls.samples].sort((a, b) => a - b);
                        const middle = Math.floor(times.length / 2);
                        const median = times.length % 2 ? times[middle] : (times[middle - 1] + times[middle]) / 2;
                        b.append(node("p", "range", `Eq-only registry control · ${median.toFixed(3)} ms median. Same graph, full search output and physical scoring calls; full observations retained in the report.`));
                    }
                }
                if (w.native_backend === "stream") {
                    const fused = w.native_stream_graph_policy === "owner-links-fused";
                    const linked = fused || w.native_stream_graph_policy === "owner-links";
                    b.append(node("p", "range", fused
                        ? "Stream graph · fused owner links. Full match payloads retained; direct projection and scoring change ownership and callback timing."
                        : linked
                          ? "Stream graph · owner links. Full match payloads retained; match IDs, batch publication and physical scoring calls differ from ordinary joins."
                          : "Stream graph · ordinary joins."));
                    for (const [key, label, detail] of [
                        ["ordinary_stream_controls", "Ordinary-join control", "Same final assignment, score and logical search work; full observations retained in the report."],
                        ["owner_link_controls", "Owner-link v1 control", "Same full search output and physical scoring calls; full observations retained in the report."],
                    ]) {
                        const controls = w[key];
                        if (!linked || !controls?.samples?.length) continue;
                        const times = [...controls.samples].sort((a, b) => a - b);
                        const middle = Math.floor(times.length / 2);
                        const median = times.length % 2 ? times[middle] : (times[middle - 1] + times[middle]) / 2;
                        b.append(node("p", "range", `${label} · ${median.toFixed(3)} ms median. ${detail}`));
                    }
                    b.append(node("p", "range", `Physical scoring calls · Rust ${n(w.rust.score_calls.min)}–${n(w.rust.score_calls.max)} per solve.`));
                }
                if (Array.isArray(w.reference_score) && !Array.isArray(w.optimal_score)) {
                    b.append(node("p", "range", `Feasible reference: ${w.reference_score[0]}hard/${w.reference_score[1]}soft. Optimality is not established.`));
                }
                if (Array.isArray(w.quality_floor)) {
                    b.append(node("p", "range", `Required quality floor: ${w.quality_floor[0]}hard/${w.quality_floor[1]}soft.`));
                }
                b.append(node("p", "range", `Work · Rust ${n(w.rust.evaluations.min)}–${n(w.rust.evaluations.max)} logical evaluations · Java ${n(w.java.score_calls.min)}–${n(w.java.score_calls.max)} scoring calls.`));
                if (cold && w.rust.construction_steps) {
                    b.append(node("p", "range", `Construction · ${n(w.rust.construction_steps.min)}–${n(w.rust.construction_steps.max)} placements per solve; included in Rust evaluation work, separate from local-search steps.`));
                }
                b.append(node("p", "range", `Limits · ${n(w.case.steps)} local-search steps and ${n(w.case.evaluations)} evaluations per solve${cold ? " across construction and search" : ""}. Different move policies and work.`));
            } else {
                b.append(node("p", "range", `${kind === "stream" ? "Terminal weighting calls" : "Score calls"} · Rust ${n(w.rust.score_calls.min)}–${n(w.rust.score_calls.max)} · Java ${n(w.java.score_calls.min)}–${n(w.java.score_calls.max)}`));
            }
        }
        if (kind === "repair") {
            b.append(node("p", "range", `Ruined group: ${n(w.ruined_count)} variables. Repairs · Rust ${n(w.rust.repairs.min)}–${n(w.rust.repairs.max)} · Java ${n(w.java.repairs.min)}–${n(w.java.repairs.max)}. Sampling and scoring work differ.`));
        }
        if (kind === "list_repair") {
            b.append(node("p", "range", `Requested group range: ${n(w.minimum_ruined)}–${n(w.maximum_ruined)} values. Repairs · Rust ${n(w.rust.repairs.min)}–${n(w.rust.repairs.max)} · Java ${n(w.java.repairs.min)}–${n(w.java.repairs.max)}. Unassigned · Rust ${n(w.rust.unassigned)}, Java ${n(w.java.unassigned)}. Sampling and scoring work differ.`));
        }
        box.append(b);
    });
    target("benchmark-caption").textContent = missing
        ? "No timing data is inferred from test duration."
        : `Medians per ${solver ? "solve" : "iteration"} · ${p.config.forks} fresh processes × ${p.config.samples} samples × ${n(p.config.iterations)} ${solver ? "solves" : "iterations"}; ${p.config.warmup} warmups per process. Measured ${timestamp(p.generated_at)}. Result checksums match.`;
    if (heuristic && !missing) {
        target("benchmark-caption").textContent = `At most ${p.config.steps} completed steps · ${p.config.forks} fresh processes × ${p.config.samples} measured solves; ${p.config.warmup} warmup solves per process. Measured ${timestamp(p.generated_at)}. Every returned assignment independently checked.`;
    }
    if (kind === "repair" && !missing) {
        target("benchmark-caption").textContent = `${p.config.steps} verified repairs per solve · ${p.config.forks} fresh processes × ${p.config.samples} measured solves; ${p.config.warmup} warmups. Measured ${timestamp(p.generated_at)}. Independent assignment, score and quality checks; different PRNGs and reconstruction orders.`;
    }
    if (kind === "mixed" && !missing) {
        target("benchmark-caption").textContent = `Mixed ${p.config.start_modes?.includes("cold") ? "warm/cold" : "warm-start"} search with per-case limits · ${p.config.forks} fresh processes × ${p.config.samples} samples × ${n(p.config.iterations)} solves; ${p.config.warmup} warmup batches. Measured ${timestamp(p.generated_at)}. Every solve checked independently; Java samples shared between native-backend comparisons.`;
    }
    if (kind === "list_repair" && !missing) {
        target("benchmark-caption").textContent = `${p.config.steps} verified repairs per solve · ${p.config.forks} fresh processes × ${p.config.samples} measured solves; ${p.config.warmup} warmups. Measured ${timestamp(p.generated_at)}. Required and optional lists; group ranges start at one. Independent score and optimum checks.`;
    }
    if (kind === "construction" && !missing) {
        target("benchmark-caption").textContent = `Complete construction · ${p.config.forks} fresh processes × ${p.config.samples} measured solves; ${p.config.warmup} warmup solves per process. Measured ${timestamp(p.generated_at)}. Every output independently checked; native assignments also match greedy reconstruction.`;
    }
    if (kind === "list" && !missing) {
        target("benchmark-caption").textContent = `Construction completes all insertions; route search uses at most ${p.config.steps} steps${p.config.two_opt_score_limit ? `; two-opt also has a ${n(p.config.two_opt_score_limit)} score-evaluation budget` : ""} · ${p.config.forks} fresh processes × ${p.config.samples} measured solves; ${p.config.warmup} warmups per process. Measured ${timestamp(p.generated_at)}. Every returned assignment independently checked.`;
    }
    if (kind === "kopt" && !missing) {
        target("benchmark-caption").textContent = `At most ${p.config.steps} steps and ${n(p.config.evaluations)} score evaluations · ${p.config.forks} fresh processes × ${p.config.samples} measured solves; ${p.config.warmup} warmups per process. Measured ${timestamp(p.generated_at)}. Rust uses a ${n(p.config.native_window)}-candidate window or at most ${n(p.config.native_attempts_per_step || p.config.evaluations)} direct proposals per step; Java uses random selection. Work and sampling differ.`;
    }
    if ((kind === "pillar" || kind === "concurrent") && !missing) {
        target("benchmark-caption").textContent = `Medians per transaction (batch averages) · ${p.config.forks} fresh processes × ${p.config.samples} samples × ${n(p.config.iterations)} transactions; ${p.config.warmup} warmup batches per process. Measured ${timestamp(p.generated_at)}. Trial, undo, committed states and batch checksums independently checked.`;
    }
    if ((kind === "sequence" || kind === "aggregate") && !missing) {
        target("benchmark-caption").textContent = `Medians per update/restore pair, including full observations (batch averages) · ${p.config.forks} fresh processes × ${p.config.samples} samples × ${n(p.config.iterations)} pairs; ${p.config.warmup} warmup batches per process. Measured ${timestamp(p.generated_at)}. Every trial, restored and committed result checked.`;
    }
    target("benchmark-method").textContent =
        p.methodology ||
        "Not measured. The benchmark will record its workload, warmup, sample counts, inputs, machine, and runtime versions.";
    target("environment").textContent = p.environment
        ? `${p.environment.cpu || "CPU not recorded"}\n${p.environment.platform || "Platform not recorded"}\n\n${p.environment.rust || "Rust version not recorded"}\n\n${p.environment.java || "Java version not recorded"}`
        : "Environment not recorded yet.";
}
function renderDifferential() {
    const box = $("differential");
    box.replaceChildren();
    const decimals = report.differential?.decimal_checks;
    const termination = report.solver_differential?.termination_checks;
    const trends = report.solver_differential?.score_trend_checks;
    const pooled = report.solver_differential?.pooled_construction_checks;
    const valueQueue = report.solver_differential?.value_queue_construction_checks;
    const mixedSearch = report.solver_differential?.mixed_solver_checks;
    const membership = report.stream_differential?.membership_join_checks;
    const membershipDetail = membership ? ` Membership joins (seed ${membership.seed}): ${n(membership.passed)} / ${n(membership.total)} cases and ${n(membership.observations)} states agree for distinct non-null members. Another ${n(membership.native_edge_checks?.passed)} native cases check duplicate/null members against Python. Of ${n(membership.boundary_probes?.total)} fixed Java boundary probes, ${n(membership.boundary_probes?.diverged)} differ: Java can multiply duplicate-member matches and skip unpartitioned null matches; Rust keeps one pair. No membership timing claim.` : "";
    const conditional = report.stream_differential?.conditional_collector_checks;
    const conditionalDetail = conditional ? ` Separate conditional-aggregation checks (seed ${conditional.seed}): ${n(conditional.passed)} / ${n(conditional.total)} cases and ${n(conditional.observations)} states agree with Java and independent Python scores, including groups with only rejected values, key changes and undo. Java recalculates each score; no conditional timing claim.` : "";
    const oscillation = report.solver_differential?.oscillation_checks;
    const oscillationDetail = oscillation?.total_order_controls
        ? `Explicit encounter-order policies compare highest score and both strategic references across four step prefixes. ${n(oscillation.native_reference_passed)} native cases match independent Python; ${n(oscillation.diverged)} differ from Community. Existing defaults and total-order modes remain unchanged: their separate control cohort agrees in ${n(oscillation.total_order_controls.passed)} / ${n(oscillation.total_order_controls.total)} cases, with ${n(oscillation.total_order_controls.diverged)} ordering differences. Public Java metrics check selected score and complete accepted candidate count; retained best identity/score is checked separately. No general trace, timing or upstream-method credit.`
        : `Highest-score and both strategic oscillation references across four step prefixes. ${n(oscillation?.native_reference_passed)} native cases match Python; ${n(oscillation?.diverged)} differ from Community and are excluded from agreement counts. Public Java metrics verify the last selected score and complete accepted candidate set. Community can prefer a later exact-reference candidate; these native total-order modes prefer a softer-level improvement. Retained best identity/score is checked separately. No timing or upstream-method credit.`;
    const greatDeluge = report.solver_differential?.great_deluge_checks;
    const delugeDetail = greatDeluge?.selected_policy === "binary64_ratio"
        ? `Explicit binary64 ratio arithmetic agrees in ${n(greatDeluge.passed)} / ${n(greatDeluge.total)} original cases and ${n(greatDeluge.boundary_probes?.agreed)} / ${n(greatDeluge.boundary_probes?.total)} boundary prefixes. Fixed rain is unchanged. Exact arithmetic remains the default: its complete controls retain ${n(greatDeluge.exact_controls?.passed)} / ${n(greatDeluge.exact_controls?.total)} agreements and ${n(greatDeluge.exact_controls?.boundary_probes?.diverged)} differing prefixes. Another ${n(greatDeluge.arithmetic_probes?.passed)} / ${n(greatDeluge.arithmetic_probes?.total)} ratio/budget probes agree. Native scoped/stream results, counters and reasons match independent Python. Java final scores and assignments are checked; its internal thresholds and traces are unobserved. The selected policy requires integer initial water fitting signed 64 bits; first-step conversion can round down. No general floating, overflow or decimal-search equivalence claim.`
        : `Fixed rain and three ratios, first/best selection and rejected fallback. Rust scoped/stream results, counters and reasons match exact Python arithmetic in ${n(greatDeluge?.native_reference_passed)} cases. ${n(greatDeluge?.diverged)} ratio case differs from Java and is not an agreement pass: Rust reaches score (0,9) at step 11; Java at step 13. A separate ${n(greatDeluge?.boundary_probes?.total)}-prefix probe confirms this boundary (${n(greatDeluge?.boundary_probes?.diverged)} different). Floating accumulation is a supported explanation, not an observation of Java internals. No general floating/overflow or decimal search equivalence claim.`;
    const stepCounting = report.solver_differential?.step_counting_checks;
    const averages = report.stream_differential?.average_collector_checks;
    const ranges = report.stream_differential?.range_collector_checks;
    const balance = report.stream_differential?.load_balance_checks;
    const wrappingBalance = balance?.arithmetic_policy?.startsWith("wrapping64-moments-");
    const binaryBalance = balance?.arithmetic_policy === "wrapping64-moments-binary64-root";
    const balancePolicyDetail = binaryBalance
        ? `Selected binary64 root arithmetic uses wrapping 64-bit moments and a floating square root, then rounds its exact binary value. Exact-default controls retain ${n(balance.exact_controls?.passed)} / ${n(balance.exact_controls?.total)} Java agreements; wrapping moments with an exact root retain ${n(balance.exact_root_controls?.passed)} / ${n(balance.exact_root_controls?.total)}. Both controls keep complete observations and differences. The large-offset cancellation case agrees under the selected policy. Typed negative-radicand errors and this operation order define the native policy; they do not establish general Java arithmetic or nonfinite equivalence.`
        : wrappingBalance
            ? `Selected wrapping64-moments-exact-root arithmetic wraps signed 64-bit loads, totals and squared moments, then rounds an exact widened rational root. Exact-default controls retain ${n(balance.exact_controls?.passed)} / ${n(balance.exact_controls?.total)} Java agreements and their complete observations. Large-offset floating cancellation remains unresolved. A typed negative-radicand result defines the native policy; it makes no claim about Java nonfinite behavior.`
            : "Exact native totals and squared deviations avoid Java overflow and large-offset cancellation.";
    const sequences = report.stream_differential?.sequence_collector_checks;
    const durations = report.stream_differential?.duration_collector_checks;
    const periods = report.stream_differential?.period_collector_checks;
    const extrema = report.stream_differential?.extrema_collector_checks;
    const composition = report.stream_differential?.composition_collector_checks;
    const setSum = report.stream_differential?.set_sum_collector_checks;
    const bigNumbers = report.stream_differential?.big_number_collector_checks;
    const mixed = report.stream_differential?.mixed_stream_checks;
    const followers = report.stream_differential?.follower_checks;
    const collectionFollowers = report.stream_differential?.collection_follower_checks;
    const listElements = report.stream_differential?.list_element_checks;
    const concurrent = report.stream_differential?.concurrent_checks;
    const existence = report.stream_differential?.equality_existence_checks;
    const existenceDetail = existence ? ` Separate equality-existence checks (seed ${existence.seed}): ${n(existence.passed)} / ${n(existence.total)} cases and ${n(existence.observations)} states agree on retained counts and scores for exists/not-exists with zero, one, or two equality keys. Python checks native left identities and payloads through duplicate matches, first/last removals and undo. Java recalculates each score; no existence timing claim.` : "";
    const ordered = report.stream_differential?.ordered_join_checks;
    const orderedDetail = ordered ? ` Separate ordered/overlap join checks (seed ${ordered.seed}): ${n(ordered.passed)} / ${n(ordered.total)} cases and ${n(ordered.observations)} states agree on match counts and weighted scores across five relations with zero, one, or two equality keys, plus interval overlaps with and without equality partitions. Python also checks native match identities and payloads through edits and undo. Java recalculates each score; no ordered/overlap join timing claim.` : "";
    const mixedDetail = mixed ? ` Separate mixed-model checks (seed ${mixed.seed}): ${n(mixed.passed)} / ${n(mixed.total)} cases, ${n(mixed.observations)} states, and ${n(mixed.operations)} basic/list operations agree on scores, assignments, and owner/index/neighbor views through commit and undo. Java recalculates each score; no mixed search or timing claim.` : "";
    const bigNumberDetail = bigNumbers ? ` Separate arbitrary-precision checks (seed ${bigNumbers.seed}): ${n(bigNumbers.passed)} / ${n(bigNumbers.total)} cases and ${n(bigNumbers.observations)} observations match exact integer values, decimal coefficients, and scales.` : "";
    const durationDetail = durations ? ` Separate duration checks (seed ${durations.seed}): ${n(durations.passed)} / ${n(durations.total)} cases, ${n(durations.observations)} observations, including ${n(durations.expected_overflow_rejections)} expected update or finish overflows. Signed nanosecond results agree exactly.` : "";
    const averageDetail = averages ? ` Separate average checks (seed ${averages.seed}): ${n(averages.passed)} / ${n(averages.total)} cases, ${n(averages.observations)} observations, including ${n(averages.expected_overflow_rejections)} expected overflow rejections. Floating-point results match bit for bit.` : "";
    for (const [d, label, detail] of [
        [report.differential || {}, "generated integer score cases agree with Java", "Boundary values, structural scores, arithmetic, and ordering."],
        ...(decimals ? [[{...decimals, status: report.differential.status}, "decimal score cases agree with Java and independent Python arithmetic", `Three fixed decimal families and five Bendable shapes (including empty groups): dimension-checked parsing, exact coefficients and scales, formatting, feasibility, comparison and arithmetic; ${n(decimals.expected_errors)} expected rejections. Selected finite Java factors map to native exact Decimal factors; powers are nonnegative integers. No general floating-input or decimal timing claim.`]] : []),
        [report.solver_differential || {}, "complete planning cases agree with Java and independent enumeration", "Finite domains, pinned variables, hard conflicts and soft preferences; movable initial values reset before both solves."],
        ...(pooled ? [[{...pooled, status: report.solver_differential.status}, "pooled construction cases have equal native and Community final scores", `${n(pooled.native_reference_passed)} native assignments also match independent full-candidate greedy reconstruction. Public Java CHEAPEST_INSERTION uses original entity/value order and NEVER early pick. Exact optima: Rust ${n(pooled.optimal?.rust)}, Java ${n(pooled.optimal?.java)} of ${n(pooled.total)}. Pins and signed weights are checked. No Java trace, list/mixed comparison, dedicated timing or adapted-method credit.`]] : []),
        ...(valueQueue ? [[{...valueQueue, status: report.solver_differential.status}, "value-queue construction cases have equal native and Community final scores", `${n(valueQueue.native_reference_passed)} native assignments also match independent receiver reconstruction. Public Java ALLOCATE_TO_VALUE_FROM_QUEUE uses original entity/value order and NEVER early pick. Exact optima: Rust ${n(valueQueue.optimal?.rust)}, Java ${n(valueQueue.optimal?.java)} of ${n(valueQueue.total)}. Pins and signed weights are checked. No Java trace, heterogeneous/optional/list/mixed comparison, dedicated timing or adapted-method credit.`]] : []),
        ...(trends ? [[{...trends, status: report.solver_differential.status}, "score-trend cases agree with Community and independent optima", "Four branch exploration orders with ANY and ONLY_DOWN/ANY trends over 64 original graph cases with pins and signed soft preferences. Final assignments and scores are independently validated. No traversal, internal bound representation, timing or adapted-method credit."]] : []),
        ...(termination ? [[{...termination, status: report.solver_differential.status}, "termination cases agree with Community and independent search", "Step, stagnation, score-target and feasibility limits, including delayed improvements after rejected fallback and recovery. Java/Rust final solutions and scores agree; Python separately checks Rust scoped and stream counters and termination reasons. Java internal counters, traces, list/mixed search and timing are not compared."]] : []),
        ...(oscillation ? [[{...oscillation, status: report.solver_differential.status}, "forager cases agree with Community and independent ranking", oscillationDetail]] : []),
        ...(greatDeluge ? [[{...greatDeluge, status: report.solver_differential.status}, "Great Deluge search cases agree with Community and independent search", delugeDetail]] : []),
        ...(stepCounting ? [[{...stepCounting, status: report.solver_differential.status}, "step-counting search cases agree with Community and independent search", "Five counting modes, three threshold periods, first/best accepted selection and rejected fallback. Java/Rust final assignments and scores agree; Python separately verifies Rust scoped/stream counters and termination reasons. Java internal counters and traces are not compared."]] : []),
        ...(mixedSearch ? [[{...mixedSearch, status: report.solver_differential.status}, "mixed planning cases checked for valid solutions and scores", `Independent enumeration checks ${n(mixedSearch.reference_assignments)} joint depot/route assignments. Optima reached: Rust exact ${n(mixedSearch.optimal.exact)}/${n(mixedSearch.total)}, Community BRUTE_FORCE ${n(mixedSearch.optimal.java)}/${n(mixedSearch.total)}, Rust scoped and stream local search ${n(mixedSearch.optimal.scoped)}/${n(mixedSearch.total)} each. The coupled cases expose quality differences; these are not equal-score passes. Native local search uses joint moves and a 20-step cap. Different search strategies and starting states; no timing or Java trace comparison.`]] : []),
        [report.stream_differential || {}, "stream cases agree with Java score updates", `${n(report.stream_differential?.observations || 0)} states cover incremental candidates, commits and undo. Python separately checks native conflict IDs and group sums. Java Community comparison exposes scores only.${averageDetail}${durationDetail}${bigNumberDetail}${mixedDetail}${orderedDetail}${existenceDetail}${conditionalDetail}${membershipDetail}`],
        ...(setSum ? [[{...setSum, status: report.stream_differential.status}, "set, merged-map and checked-sum cases agree with Java and independent Python", `${n(setSum.observations)} observations cover duplicates, replacements, removals and inverse-update trials. Includes ${n(setSum.expected_overflow_rejections)} expected sum-overflow rejections; these are rejected operations, not successful updates. Checked sums validate replacement difference and final total. Merged maps use wrapping 32-bit addition and retain zero-valued keys until the final occurrence is removed. Unordered sets normalize iteration order. Both runtimes stop at the first error; no Java post-error recovery, general merge-function or timing claim.`]] : []),
        ...(composition ? [[{...composition, status: report.stream_differential.status}, "collector-composition cases agree with Java and independent Python", `${n(composition.observations)} states compare two-, three- and four-collector results and count-result mapping through duplicate values, replacements, removals and inverse-update trials. Minima, maxima, counts and every average bit agree. Integer inputs keep intermediate sums and differences representable. Native graph rollback and transformed search scores have separate regression checks. No callback-count, callback-order or timing claim.`]] : []),
        ...(extrema ? [[{...extrema, status: report.stream_differential.status}, "minimum/maximum cases satisfy public selection rules", `${n(extrema.observations)} states check active selected identities, captured payloads and independently calculated extreme keys through duplicate inputs, replacement, removal and inverse-update trials. Java permits any tied value; Rust chooses the lowest stable ID. ${n(extrema.tie_choice_differences)} valid tied selections differ in identity, with raw results retained. This is agreement on selection rules, not exact tie identity or solver performance.`]] : []),
        ...(periods ? [[{...periods, status: report.stream_differential.status}, "calendar-period cases agree with Java and independent Python", `${n(periods.observations)} observations check all signed year, month and day fields through add, replace, remove and inverse-update trials. Includes ${n(periods.expected_overflow_rejections)} expected arithmetic-overflow rejections; these are agreement on rejection, not successful updates. Replacement checks subtraction before addition, even for identical periods. Successful states also match independent full sums. No automatic normalization or conversion to fixed durations. Both runtimes stop at the first error; no Java post-error state, date application or timing claim.`]] : []),
        ...(balance ? [[{...balance, status: report.stream_differential.status}, wrappingBalance ? "load-balance cases agree under explicit wrapping arithmetic" : "load-balance cases agree with Java and independent Python", `${n(balance.native_reference_passed)} / ${n(balance.total)} native cases match independent load maps and unfairness across ${n(balance.observations)} states. All ${n(balance.ordinary_passed)} bounded cases agree. ${n(balance.divergent_cases)} cases differ and ${n(balance.java_error_cases)} end in Java errors; neither receives agreement credit. ${balancePolicyDetail} Unfairness rounds to six significant digits; trailing-zero representations normalize numerically. Initial loads must agree per active key; zero-load keys participate. Original inputs, full outputs and differences are retained. No Java scale, live-map or timing claim.`]] : []),
        ...(ranges ? [[{...ranges, status: report.stream_differential.status}, "connected-range cases agree with Java and independent Python", `${ranges.arithmetic_policy === "wrapping32" ? `Explicit 32-bit wrapping lengths are selected. Default exact-metric controls retain ${n(ranges.exact_controls?.passed)} / ${n(ranges.exact_controls?.total)} agreements. ` : ""}${n(ranges.native_reference_passed)} / ${n(ranges.total)} native cases match independent interval calculations across ${n(ranges.observations)} states. All ${n(ranges.ordinary_passed)} distinct-identity, positive-length cases agree. ${n(ranges.divergent_cases)} cases differ and ${n(ranges.java_error_cases)} end in Java errors; neither receives agreement credit. ${ranges.arithmetic_policy === "wrapping32" ? "Integer length overflow now agrees under the selected policy. Java still can retain duplicate-object occurrences after removal and report inconsistent empty-range overlap flags." : "Java can retain duplicate-object occurrences after removal, report inconsistent empty-range overlap flags, and overflow integer lengths."} Raw cases and differences are retained. Empty-range overlap sentinels map to native absence; native results use immutable snapshots. No live-object hash, Java iteration-order or timing claim.`]] : []),
        ...(sequences ? [[{...sequences, status: report.stream_differential.status}, "consecutive-sequence cases agree with Java and independent Python", `${n(sequences.observations)} states check item membership, sequence and break lengths, counts, endpoints and flags through add, replace, remove and trial/undo. Equal-index items may have different tie ordering; Rust uses stable IDs. ${sequences.arithmetic_policy === "wrapping32" ? `Explicit 32-bit wrapping distances agree on the original MIN/MAX boundary, including break length -1. Default exact-metric controls still return 4,294,967,295 and retain ${n(sequences.exact_controls?.boundary_divergences)} separate overflow difference.` : `${n(sequences.boundary_divergences)} separate integer-overflow boundary differs: Java break length -1, Rust 4,294,967,295. This boundary receives no agreement credit.`} No live-view, mutable-object aliasing or timing claim.`]] : []),
        ...(followers ? [[{...followers, status: report.stream_differential.status}, "follower cases agree with Java and independent Python", `${n(followers.observations)} states and ${n(followers.operations)} operations check dynamic leader references and fixed shared followers, including null values, composites, commit and undo. Every reference, derived value and ordinary score is checked. Java maintains shadow fields through public moves and recalculates scores; Rust uses reference streams. The pinned interface-based constraint includes null-valued rows. No supplier-count, search or timing comparison.`]] : []),
        ...(collectionFollowers ? [[{...collectionFollowers, status: report.stream_differential.status}, "collection follower cases agree with Java and independent Python", `${n(collectionFollowers.observations)} states and ${n(collectionFollowers.operations)} operations check minimum derived values across fixed nonempty leader collections. Cases include repeated references, null values, equal-code ties, and UTF-16 string ordering. Every derived identity and ordinary score is checked through commit and undo. Java recomputes observed scores. No dependency-list mutation, supplier-count, search or timing comparison.`]] : []),
        ...(listElements ? [[{...listElements, status: report.stream_differential.status}, "list-element cases agree with Java and independent Python", `${n(listElements.observations)} states and ${n(listElements.operations)} operations check ordered start/end chains and nullable owner aggregates through list moves, basic changes, composites and undo. Java move-driven shadows are observed unchanged; ordered scores come from a separate fully updated solution. Mixed cases compare derived values only. Includes empty lists and zero values. No supplier-count, search or timing comparison.`]] : []),
        ...(concurrent ? [[{...concurrent, status: report.stream_differential.status}, "concurrent-schedule cases agree with Java and independent Python", `${n(concurrent.native_reference_passed)}/${n(concurrent.total)} native cases match Python across ${n(concurrent.observations)} states and ${n(concurrent.operations)} operations. Java supplied ${n(concurrent.java_observations)} observed states; ${n(concurrent.java_error_cases)} cases ended in runtime errors and ${n(concurrent.divergent_cases)} cases had differing observations. Errors receive no agreement credit. A partial-group scoring error is also reproduced using unchanged pinned fixtures. Checks cover shared times, cycles, recovery and undo. Concurrent scoring now uses an owned cache, checked against full native evaluation and Python. Cached trial snapshots restore without supplier calls. No general Java supplier-count, search or timing comparison.`]] : []),
        [{...report.concurrent_performance?.validation, seed: report.concurrent_performance?.config?.seed, status: report.concurrent_performance?.status}, "concurrent shadow transactions agree with Java and independent Python", "Fully assigned schedules and distinct move positions: all nine shadow fields checked through trial, undo and commit. Cache snapshots versus public Java shadow updates; no scoring or search timing. The separate partial-group correctness errors remain visible above."],
        [{...report.pillar_performance?.validation, seed: report.pillar_performance?.config?.seed, status: report.pillar_performance?.status}, "pillar transaction cases agree with Java and an independent reference", "Supplied groups and selected columns: change, swap, optional values and subgroups. Trial, undo and committed states match. No automatic grouping or solver search comparison."],
        ...(report.list_differential?.value_metadata ? [[{...report.list_differential.value_metadata, seed: report.list_differential.seed, status: report.list_differential.status}, "list planning-value cases agree on identity membership", `${n(report.list_differential.value_metadata.native_reference_passed)} / ${n(report.list_differential.value_metadata.total)} native cases match independently replayed value capture before trial and commit. Java getters are observed after execution. ${n(report.list_differential.value_metadata.divergent_cases)} cases differ, ${n(report.list_differential.value_metadata.java_error_cases)} return Java errors, and ${n(report.list_differential.value_metadata.unsupported_cases)} empty compositions are unsupported; these receive no agreement credit. Lists, neighbors and undo are also checked. Java order and multiplicity remain in raw output; comparison uses identity sets. No search or timing claim.`]] : []),
        ...(report.list_differential?.gather ? [[{...report.list_differential.gather, status: report.list_differential.status}, "list-gather cases agree with Java and an independent reference", "Ordered selections gather at original insertion boundaries, including currently unassigned members; null destinations unassign selected assigned members. Candidate, undo and committed lists and owner/index/neighbor views are checked. No pinned Java cases, move-metadata equivalence, timing or method coverage credit."]] : []),
        [report.list_differential || {}, "list-move cases agree with Java and an independent reference", "Candidate, undo and committed lists, including owner, index and previous/next neighbors. Move checks only; list-solving evidence is reported separately."],
    ]) {
        if (d.total == null) {
            box.append(node("p", "note", `${detail} Comparison not recorded yet.`));
            continue;
        }
        const p = node("p");
        p.append(node("strong", "", `${n(d.passed)} / ${n(d.total)}`), document.createTextNode(` ${label}.`));
        box.append(p, node("p", "note", `${detail} Seed ${d.seed}. Original checks; no additional upstream coverage credit.`));
        if (d.status === "stale" || !fresh)
            box.append(node("p", "note", "Previous result: measured sources have changed; rerun the oracle."));
        if (d.failures?.length)
            box.append(node("p", "note", `${n(d.failures.length)} discrepancies are recorded in the downloadable report.`));
    }
    for (const [key, label] of [
        ["repair_performance", " original repair cases return valid, non-regressing solutions."],
        ["list_repair_performance", " original list-repair cases return valid, non-regressing solutions."],
        ["construction_performance", " original construction cases return valid complete assignments and scores."],
        ["heuristic_performance", " original heuristic cases return valid, non-regressing solutions."],
        ["stream_performance", " original incremental-stream search cases return valid, non-regressing solutions."],
        ["list_performance", " original list-solving cases return valid complete solutions; local search retains at least its initial quality."],
        ["kopt_performance", " cyclic k-opt paired comparisons return valid, non-regressing solutions."],
    ]) {
        const h = report[key] || {};
        if (h.validation) {
            const v = h.validation, p = node("p");
            p.append(node("strong", "", `${n(v.passed)} / ${n(v.total)}`), document.createTextNode(label));
            box.append(p, node("p", "note", `${n(v.equal_scores)} cases have equal Java/Rust scores. Independently proven optima reached: Rust ${n(v.optimal.rust)}, Java ${n(v.optimal.java)}. No additional upstream coverage credit.`));
            if (key === "list_repair_performance") {
                for (const [mode, detail] of Object.entries(v.by_algorithm || {})) {
                    box.append(node("p", "note", `${mode}: ${n(detail.equal_scores)} / ${n(detail.total)} equal-score cases; optima reached by Rust ${n(detail.optimal.rust)}, Java ${n(detail.optimal.java)}.`));
                }
                for (const probe of h.boundary_probes || []) {
                    box.append(node("p", "note", `Group boundary · ${probe.case.optional ? "optional" : "required"} lists, requested group ${n(probe.minimum)}–${n(probe.maximum)}: Java completed ${n(probe.java.repairs)}, Rust ${n(probe.rust.repairs)} of ${n(probe.requested_repairs)} requested repairs. Scores: Java ${probe.java.score.join("/")}, Rust ${probe.rust.score.join("/")}. Separate behavior probe; excluded from timing comparisons.`));
                }
            }
            if (key === "kopt_performance") {
                if (v.by_strategy) {
                    for (const [strategy, detail] of Object.entries(v.by_strategy)) {
                        box.append(node("p", "note", `Rust ${strategy}: ${n(detail.equal_scores)} / ${n(detail.total)} equal-score comparisons; optima reached by Rust ${n(detail.optimal.rust)}, Java ${n(detail.optimal.java)}. Strategies use the same routes and k ranges.`));
                    }
                } else {
                    for (const [maximum, detail] of Object.entries(v.by_maximum || {})) {
                        box.append(node("p", "note", `K-opt 2–${maximum}: ${n(detail.equal_scores)} / ${n(detail.total)} equal-score cases; optima reached by Rust ${n(detail.optimal.rust)}, Java ${n(detail.optimal.java)}.`));
                    }
                }
                for (const probe of h.boundary_probes || []) {
                    box.append(node("p", "note", `Exact k=${probe.minimum} · ${n(probe.case.points.length)} values: Java made ${n(probe.java.score_calls)} score calculations, Rust ${n(probe.rust.score_calls)}. Scores: Java ${probe.java.score.join("/")}, Rust ${probe.rust.score.join("/")}. Separate behavior probe; excluded from timing comparisons.`));
                }
            }
            if (key === "construction_performance") {
                box.append(node("p", "note", `${n(v.native_reference_passed)} native assignments match independent greedy reconstruction; ${n(v.equal_assignments)} assignments match Community. No new upstream-method credit.`));
                const names = {sequential: "Sequential", pooled: "Pooled cheapest insertion", "value-queue": "Cycling value queue"};
                for (const [mode, detail] of Object.entries(v.by_algorithm || {})) {
                    box.append(node("p", "note", `${names[mode] || mode}: ${n(detail.equal_scores)} / ${n(detail.total)} equal-score cases; optima reached by Rust ${n(detail.optimal.rust)}, Java ${n(detail.optimal.java)}.`));
                }
            }
            if (key === "list_performance" && v.by_mode) {
                const names = {construct: "Construction", hill: "Relocation hill climbing", "two-opt": "Two-opt"};
                for (const [mode, detail] of Object.entries(v.by_mode)) {
                    box.append(node("p", "note", `${names[mode] || mode}: ${n(detail.equal_scores)} / ${n(detail.total)} equal-score cases; independently proven optima reached by Rust ${n(detail.optimal.rust)}, Java ${n(detail.optimal.java)}. Candidate sampling and move order can differ.`));
                }
            }
            if (h.status === "stale" || !fresh) box.append(node("p", "note", "Previous solving result: measured sources have changed."));
        }
    }
}
async function reload() {
    if (loading) return;
    const reloadHadFocus = document.activeElement === $("reload");
    loading = true;
    $("reload").disabled = true;
    try {
        const response = await fetch("data/report.json", { cache: "no-store" });
        if (!response.ok)
            throw new Error(
                `Report not available (${response.status}). Run python3 scripts/compat_report.py, then reload results.`,
            );
        const data = await response.json();
        if (data.schema_version !== 1 || !Array.isArray(data.families))
            throw new Error(
                "Unrecognized report format. Regenerate the report with the current scripts.",
            );
        fresh = true;
        completionFresh = true;
        completionLive = false;
        nativeFresh = false;
        try {
            const state = await fetch("api/freshness", { cache: "no-store" });
            if (state.ok) {
                const snapshot = await state.json();
                completionLive = typeof snapshot.completion_digest === "string" && typeof snapshot.verification_digest === "string";
                fresh = snapshot.verification_digest === data.verification_digest;
                completionFresh = snapshot.completion_digest === data.completion?.model_digest;
                nativeFresh = typeof snapshot.native_completion_digest === "string" && snapshot.native_completion_digest === data.native_readiness?.model_digest;
            }
        } catch {
            /* Static hosting still shows the generation timestamp. */
        }
        report = data;
        selected = report.families.some((f) => f.name === selected)
            ? selected
            : report.families[0]?.name;
        if (!selected)
            throw new Error(
                "The inventory contains no test families. Regenerate the report.",
            );
        if (lastStamp !== report.generated_at) {
            render();
            lastStamp = report.generated_at;
        } else {
            renderCompletion();
            renderNativeReadiness();
            renderBenchmarks();
            renderBenchmarks("solver");
            renderBenchmarks("mixed");
            renderBenchmarks("construction");
            renderBenchmarks("repair");
            renderBenchmarks("list_repair");
            renderBenchmarks("heuristic");
            renderBenchmarks("list");
            renderBenchmarks("kopt");
            renderBenchmarks("stream");
            renderBenchmarks("pillar");
            renderBenchmarks("concurrent");
            renderBenchmarks("aggregate");
            renderBenchmarks("sequence");
            renderDifferential();
        }
        $("updated").textContent =
            `Report updated ${timestamp(report.generated_at)} · reloads every 15s`;
        const warnings = [];
        if (!fresh)
            warnings.push(
                "Source files changed since verification. Coverage shown is the last saved result. Run bash scripts/check.sh to refresh it.",
            );
        if (!completionFresh)
            warnings.push("The completion model changed. Regenerate the report to update its progress estimate.");
        if (!report.verification?.successful)
            warnings.push(
                "The last Rust verification was incomplete or failed. Only observed passing adaptations count toward coverage; inspect the full report for failures.",
            );
        $("message").hidden = warnings.length === 0;
        $("message").textContent = warnings.join(" ");
    } catch (error) {
        $("message").hidden = false;
        $("message").textContent =
            `${error.message}${report ? " The last loaded report remains visible." : ""}`;
        $("updated").textContent = report
            ? `Last loaded ${timestamp(report.generated_at)} · refresh failed`
            : "No verified report loaded";
    } finally {
        loading = false;
        $("reload").disabled = false;
        if (reloadHadFocus && document.activeElement === document.body)
            $("reload").focus({ preventScroll: true });
    }
}
$("reload").addEventListener("click", reload);
$("test-search").addEventListener("input", () => {
    limit = 40;
    if (report) renderTests();
});
$("test-status").addEventListener("change", () => {
    limit = 40;
    if (report) renderTests();
});
$("show-more").addEventListener("click", () => {
    limit += 40;
    renderTests();
    if ($("show-more").hidden)
        $("test-list")
            .querySelector(".test-item:last-child a")
            ?.focus({ preventScroll: true });
});
reload();
setInterval(reload, 15000);
