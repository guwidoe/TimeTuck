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
    fresh = true;
const n = (value) => format.format(value);
const pct = (value) => (value == null ? "—" : `${value.toFixed(2)}%`);
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
    $("overall-percent").textContent = pct(s.percent);
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
                `${f.name}: ${f.passed} of ${f.total} passed, ${pct(f.percent)}`,
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
                    pct(f.percent),
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
    renderDetail();
    renderBenchmarks();
    renderBenchmarks("solver");
    renderBenchmarks("heuristic");
    renderBenchmarks("list");
    renderBenchmarks("stream");
    renderDifferential();
    $("inventory-note").textContent =
        `${n(s.total)} annotated methods in ${n(report.inventory.source_files)} Java test-source files at ${report.reference.commit.slice(0, 12)}. ${report.inventory.limitations}`;
    $("updated").textContent =
        `Report updated ${timestamp(report.generated_at)} · reloads every 15s`;
    $("content").hidden = false;
    restoreFocus(focused);
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
    const heuristic = kind === "heuristic" || kind === "list" || kind === "stream";
    const prefix = solver ? `${kind}-` : "";
    const target = (id) => document.getElementById(prefix + id);
    const unit = solver ? "ms" : "ns";
    const p = (solver ? report[`${kind}_performance`] : report.performance) || {},
        box = target("benchmarks");
    box.replaceChildren();
    const missing = !p.workloads?.length;
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
                node("b", "", `${w[runtime].median.toFixed(solver ? 3 : 2)} ${unit}`),
            );
            b.append(row);
        });
        b.append(
            node(
                "p",
                "range",
                `Observed ranges · Rust ${w.rust.min.toFixed(solver ? 3 : 2)}–${w.rust.max.toFixed(solver ? 3 : 2)} ${unit} · Java ${w.java.min.toFixed(solver ? 3 : 2)}–${w.java.max.toFixed(solver ? 3 : 2)} ${unit}`,
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
            b.append(node("p", "range", `${kind === "stream" ? "Terminal weighting calls" : "Score calls"} · Rust ${n(w.rust.score_calls.min)}–${n(w.rust.score_calls.max)} · Java ${n(w.java.score_calls.min)}–${n(w.java.score_calls.max)}`));
        }
        box.append(b);
    });
    target("benchmark-caption").textContent = missing
        ? "No timing data is inferred from test duration."
        : `Medians per ${solver ? "solve" : "iteration"} · ${p.config.forks} fresh processes × ${p.config.samples} samples × ${n(p.config.iterations)} ${solver ? "solves" : "iterations"}; ${p.config.warmup} warmups per process. Measured ${timestamp(p.generated_at)}. Result checksums match.`;
    if (heuristic && !missing) {
        target("benchmark-caption").textContent = `At most ${p.config.steps} completed steps · ${p.config.forks} fresh processes × ${p.config.samples} measured solves; ${p.config.warmup} warmup solves per process. Measured ${timestamp(p.generated_at)}. Every returned assignment independently checked.`;
    }
    if (kind === "list" && !missing) {
        target("benchmark-caption").textContent = `Construction completes all insertions; hill climbing uses at most ${p.config.steps} steps · ${p.config.forks} fresh processes × ${p.config.samples} measured solves; ${p.config.warmup} warmups per process. Measured ${timestamp(p.generated_at)}. Every returned assignment independently checked.`;
    }
    target("benchmark-method").textContent =
        p.methodology ||
        "Not measured. The benchmark will record its workload, warmup, sample counts, inputs, machine, and runtime versions.";
    target("environment").textContent = p.environment
        ? `${p.environment.cpu}\n${p.environment.platform}\n\n${p.environment.rust}\n\n${p.environment.java}`
        : "Environment not recorded yet.";
}
function renderDifferential() {
    const box = $("differential");
    box.replaceChildren();
    for (const [d, label, detail] of [
        [report.differential || {}, "generated score cases agree with Java", "Boundary values, structural scores, arithmetic, and ordering."],
        [report.solver_differential || {}, "complete planning cases agree with Java and independent enumeration", "Finite domains, pinned variables, hard conflicts and soft preferences; movable initial values reset before both solves."],
        [report.stream_differential || {}, "stream cases agree with Java score updates", `${n(report.stream_differential?.observations || 0)} states cover incremental candidates, commits and undo. Python separately checks native conflict IDs and group sums. Java Community comparison exposes scores only.`],
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
            box.append(node("p", "note", `${n(d.failures.length)} mismatches are recorded in the downloadable report.`));
    }
    for (const [key, label] of [
        ["heuristic_performance", " original heuristic cases return valid, non-regressing solutions."],
        ["stream_performance", " original incremental-stream search cases return valid, non-regressing solutions."],
        ["list_performance", " original list-solving cases return valid complete solutions; local search retains at least its initial quality."],
    ]) {
        const h = report[key] || {};
        if (h.validation) {
            const v = h.validation, p = node("p");
            p.append(node("strong", "", `${n(v.passed)} / ${n(v.total)}`), document.createTextNode(label));
            box.append(p, node("p", "note", `${n(v.equal_scores)} cases have equal Java/Rust scores. Independently proven optima reached: Rust ${n(v.optimal.rust)}, Java ${n(v.optimal.java)}. No additional upstream coverage credit.`));
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
        try {
            const state = await fetch("api/freshness", { cache: "no-store" });
            if (state.ok)
                fresh =
                    (await state.json()).verification_digest ===
                    data.verification_digest;
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
            renderBenchmarks();
            renderBenchmarks("solver");
            renderBenchmarks("heuristic");
            renderBenchmarks("list");
            renderBenchmarks("stream");
            renderDifferential();
        }
        $("updated").textContent =
            `Report updated ${timestamp(report.generated_at)} · reloads every 15s`;
        const warnings = [];
        if (!fresh)
            warnings.push(
                "Source files changed since verification. Coverage shown is the last saved result. Run bash scripts/check.sh to refresh it.",
            );
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
