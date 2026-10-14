//! Executable, lossless native termination projection from XML declarations.
use crate::*;
use std::{error::Error, fmt, time::Duration};
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XmlTerminationError(pub String);
impl fmt::Display for XmlTerminationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl Error for XmlTerminationError {}
fn error(s: impl Into<String>) -> XmlTerminationError {
    XmlTerminationError(s.into())
}
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct XmlTerminationTime {
    pub duration: Option<Duration>,
    pub units: TimeLimitUnits,
}
impl XmlTerminationTime {
    pub fn resolve(&self) -> Result<Option<Duration>, XmlTerminationError> {
        let units = TimeLimitSpec::Units(self.units)
            .resolve()
            .map_err(|e| error(e.to_string()))?;
        if self.duration.is_some() && units.is_some() {
            return Err(error(
                "duration and additive unit declarations cannot coexist",
            ));
        }
        Ok(self.duration.or(units))
    }
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct XmlDiminishedReturns {
    pub window: XmlTerminationTime,
    pub minimum_improvement_ratio: Option<f64>,
}
impl XmlDiminishedReturns {
    pub fn resolve(&self) -> Result<DiminishedReturnsConfig, XmlTerminationError> {
        let window = self.window.resolve()?.unwrap_or(Duration::from_secs(30));
        match self.minimum_improvement_ratio {
            Some(ratio) => DiminishedReturnsConfig::from_f64(window, ratio),
            None => DiminishedReturnsConfig::new(window, 1, 10_000),
        }
        .map_err(|e| error(e.to_string()))
    }
}
/// Composition of explicitly declared XML termination conditions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum XmlTerminationComposition {
    And,
    Or,
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct XmlTerminationSettings {
    pub composition: Option<XmlTerminationComposition>,
    /// Owned child configurations; grouping is preserved during scoped execution.
    pub terminations: Vec<XmlTerminationSettings>,
    pub spent: XmlTerminationTime,
    pub unimproved: XmlTerminationTime,
    pub best_score_limit: Option<String>,
    pub steps: Option<u64>,
    pub unimproved_steps: Option<u64>,
    pub diminished_returns: Option<XmlDiminishedReturns>,
}
fn scalar<'a>(n: &'a SolverXmlElement, key: &str) -> Result<Option<&'a str>, XmlTerminationError> {
    let mut children = n.children.iter().filter(|c| c.name == key);
    let Some(child) = children.next() else {
        return Ok(None);
    };
    if children.next().is_some() || !child.attributes.is_empty() || !child.children.is_empty() {
        return Err(error(format!("{key}: requires one scalar declaration")));
    }
    Ok(Some(child.text.trim()))
}
fn number(n: &SolverXmlElement, key: &str) -> Result<Option<u64>, XmlTerminationError> {
    scalar(n, key)?
        .map(|s| {
            s.parse::<u64>()
                .map_err(|_| error(format!("{key}: requires a nonnegative integer")))
        })
        .transpose()
}
fn time(
    n: &SolverXmlElement,
    duration: &str,
    keys: [&str; 5],
) -> Result<XmlTerminationTime, XmlTerminationError> {
    let declared = XmlTerminationTime {
        duration: scalar(n, duration)?
            .map(|s| s.parse::<XmlDuration>()?.to_std())
            .transpose()
            .map_err(|e| error(format!("{duration}: {e}")))?,
        units: TimeLimitUnits {
            milliseconds: number(n, keys[0])?,
            seconds: number(n, keys[1])?,
            minutes: number(n, keys[2])?,
            hours: number(n, keys[3])?,
            days: number(n, keys[4])?,
        },
    };
    declared.resolve()?;
    Ok(declared)
}
fn children(n: &SolverXmlElement, allowed: &[&str]) -> Result<(), XmlTerminationError> {
    if !n.text.trim().is_empty() || !n.attributes.is_empty() {
        return Err(error(format!(
            "{}: requires nested termination declarations",
            n.name
        )));
    }
    for c in &n.children {
        if !allowed.contains(&c.name.as_str()) {
            return Err(error(format!(
                "{}: unsupported executable termination setting {}",
                n.name, c.name
            )));
        }
    }
    Ok(())
}
impl XmlTerminationSettings {
    pub(crate) fn spent_declaration(
        n: &SolverXmlElement,
    ) -> Result<XmlTerminationTime, XmlTerminationError> {
        time(
            n,
            "spentLimit",
            [
                "millisecondsSpentLimit",
                "secondsSpentLimit",
                "minutesSpentLimit",
                "hoursSpentLimit",
                "daysSpentLimit",
            ],
        )
    }

    pub fn from_element(n: &SolverXmlElement) -> Result<Self, XmlTerminationError> {
        children(
            n,
            &[
                "spentLimit",
                "millisecondsSpentLimit",
                "secondsSpentLimit",
                "minutesSpentLimit",
                "hoursSpentLimit",
                "daysSpentLimit",
                "unimprovedSpentLimit",
                "unimprovedMillisecondsSpentLimit",
                "unimprovedSecondsSpentLimit",
                "unimprovedMinutesSpentLimit",
                "unimprovedHoursSpentLimit",
                "unimprovedDaysSpentLimit",
                "terminationCompositionStyle",
                "bestScoreLimit",
                "stepCountLimit",
                "unimprovedStepCountLimit",
                "diminishedReturns",
                "termination",
            ],
        )?;
        let diminished = n
            .children
            .iter()
            .filter(|c| c.name == "diminishedReturns")
            .collect::<Vec<_>>();
        if diminished.len() > 1 {
            return Err(error("duplicate diminishedReturns"));
        }
        let diminished_returns = diminished
            .first()
            .map(|n| {
                children(
                    n,
                    &[
                        "slidingWindowDuration",
                        "slidingWindowMilliseconds",
                        "slidingWindowSeconds",
                        "slidingWindowMinutes",
                        "slidingWindowHours",
                        "slidingWindowDays",
                        "minimumImprovementRatio",
                    ],
                )?;
                let d = XmlDiminishedReturns {
                    window: time(
                        n,
                        "slidingWindowDuration",
                        [
                            "slidingWindowMilliseconds",
                            "slidingWindowSeconds",
                            "slidingWindowMinutes",
                            "slidingWindowHours",
                            "slidingWindowDays",
                        ],
                    )?,
                    minimum_improvement_ratio: scalar(n, "minimumImprovementRatio")?
                        .map(|s| {
                            s.parse::<f64>().map_err(|_| {
                                error("minimumImprovementRatio: requires a positive finite number")
                            })
                        })
                        .transpose()?,
                };
                d.resolve()?;
                Ok::<_, XmlTerminationError>(d)
            })
            .transpose()?;
        let settings = Self {
            terminations: n
                .children
                .iter()
                .filter(|c| c.name == "termination")
                .map(Self::from_element)
                .collect::<Result<_, _>>()?,
            spent: Self::spent_declaration(n)?,
            unimproved: time(
                n,
                "unimprovedSpentLimit",
                [
                    "unimprovedMillisecondsSpentLimit",
                    "unimprovedSecondsSpentLimit",
                    "unimprovedMinutesSpentLimit",
                    "unimprovedHoursSpentLimit",
                    "unimprovedDaysSpentLimit",
                ],
            )?,
            composition: scalar(n, "terminationCompositionStyle")?
                .map(|s| match s {
                    "AND" => Ok(XmlTerminationComposition::And),
                    "OR" => Ok(XmlTerminationComposition::Or),
                    _ => Err(error("terminationCompositionStyle: requires AND or OR")),
                })
                .transpose()?,
            best_score_limit: scalar(n, "bestScoreLimit")?.map(str::to_owned),
            steps: number(n, "stepCountLimit")?,
            unimproved_steps: number(n, "unimprovedStepCountLimit")?,
            diminished_returns,
        };
        settings.validate()?;
        Ok(settings)
    }
    fn validate(&self) -> Result<(), XmlTerminationError> {
        for child in &self.terminations {
            child.validate()?;
        }
        self.spent.resolve()?;
        self.unimproved.resolve()?;
        if let Some(raw) = &self.diminished_returns {
            raw.resolve()?;
        }
        Ok(())
    }
    fn score<S: std::str::FromStr>(&self) -> Result<Option<S>, XmlTerminationError> {
        self.best_score_limit
            .as_deref()
            .map(|raw| {
                raw.parse::<S>().map_err(|_| {
                    error("bestScoreLimit: invalid score for the native sequence score type")
                })
            })
            .transpose()
    }
    fn local<S: std::str::FromStr>(
        &self,
        c: &mut LocalSearchConfig<'_, S>,
    ) -> Result<(), XmlTerminationError> {
        if let Some(score) = self.score()? {
            c.best_score_limit = Some(score);
        }
        if let Some(time) = self.spent.resolve()? {
            c.max_time = Some(time);
        }
        if let Some(time) = self.unimproved.resolve()? {
            c.max_unimproved_time = Some(time);
        }
        if let Some(steps) = self.steps {
            c.max_steps = steps;
        }
        if let Some(steps) = self.unimproved_steps {
            c.max_unimproved_steps = Some(steps);
        }
        if let Some(raw) = &self.diminished_returns {
            let policy = raw.resolve()?;
            c.diminished_returns.get_or_insert(policy);
        }
        Ok(())
    }
    fn ordinary(&self, c: &mut ConstructionConfig<'_>) -> Result<(), XmlTerminationError> {
        if self.best_score_limit.is_some()
            || self.unimproved.resolve()?.is_some()
            || self.unimproved_steps.is_some()
            || self.diminished_returns.is_some()
        {
            return Err(error(
                "construction phase cannot use unimproved or diminished-return termination",
            ));
        }
        if let Some(time) = self.spent.resolve()? {
            c.max_time = Some(time);
        }
        if let Some(steps) = self.steps {
            c.max_steps = Some(steps);
        }
        Ok(())
    }
    fn only_spent(&self) -> Result<Option<Duration>, XmlTerminationError> {
        if self.best_score_limit.is_some()
            || self.unimproved.resolve()?.is_some()
            || self.steps.is_some()
            || self.unimproved_steps.is_some()
            || self.diminished_returns.is_some()
        {
            return Err(error("this phase supports only XML spent termination"));
        }
        self.spent.resolve()
    }
    fn reject_phase_and(&self) -> Result<(), XmlTerminationError> {
        self.reject_nested_projection()?;
        if self.composition == Some(XmlTerminationComposition::And) {
            return Err(error(
                "phase AND requires with_sequence_extensions or with_mixed_sequence_extensions",
            ));
        }
        Ok(())
    }
    fn phase<S: std::str::FromStr>(
        &self,
        p: &mut SolverPhase<'_, S>,
    ) -> Result<(), XmlTerminationError> {
        self.reject_phase_and()?;
        match p {
            SolverPhase::LocalSearch(c) => self.local(c),
            SolverPhase::Construction(c) => self.ordinary(c),
            SolverPhase::RuinRecreate(c) => {
                if let Some(t) = self.only_spent()? {
                    c.max_time = Some(t);
                }
                Ok(())
            }
            SolverPhase::Exact(c) => {
                if let Some(t) = self.only_spent()? {
                    c.max_time = Some(t);
                }
                Ok(())
            }
            SolverPhase::Custom(c) => {
                if let Some(t) = self.only_spent()? {
                    c.max_time = Some(t);
                }
                Ok(())
            }
        }
    }
    fn mixed_phase<S: std::str::FromStr>(
        &self,
        p: &mut MixedSolverPhase<'_, S>,
    ) -> Result<(), XmlTerminationError> {
        self.reject_phase_and()?;
        match p {
            MixedSolverPhase::LocalSearch(c) => self.local(&mut c.search),
            MixedSolverPhase::Construction(c) => self.ordinary(&mut c.construction),
            MixedSolverPhase::RuinRecreate(c) => {
                if let Some(t) = self.only_spent()? {
                    c.search.max_time = Some(t);
                }
                Ok(())
            }
            MixedSolverPhase::Exact(c) => {
                if let Some(t) = self.only_spent()? {
                    c.max_time = Some(t);
                }
                Ok(())
            }
            MixedSolverPhase::Custom(c) => {
                if let Some(t) = self.only_spent()? {
                    c.max_time = Some(t);
                }
                Ok(())
            }
        }
    }
    fn with_limits<S: std::str::FromStr, R>(
        &self,
        mut limits: SolverLimits<'_, S>,
        run: impl FnOnce(SolverLimits<'_, S>) -> R,
    ) -> Result<R, XmlTerminationError> {
        self.reject_nested_projection()?;
        if self.composition == Some(XmlTerminationComposition::And) {
            return Err(error(
                "solver AND requires with_sequence_extensions or with_mixed_sequence_extensions",
            ));
        }
        if let Some(score) = self.score()? {
            limits.best_score_limit = Some(score);
        }
        if let Some(t) = self.spent.resolve()? {
            limits.max_time = Some(t);
        }
        if let Some(t) = self.unimproved.resolve()? {
            limits.max_unimproved_time = Some(t);
        }
        if let Some(steps) = self.unimproved_steps {
            limits.max_unimproved_steps = Some(steps);
        }
        let Some(steps) = self.steps else {
            return Ok(run(limits));
        };
        let old = limits
            .termination
            .copied()
            .unwrap_or(SearchTermination::Any(&[]));
        let children = [old, SearchTermination::Condition(SearchLimit::Steps(steps))];
        let expression = SearchTermination::Any(&children);
        Ok(run(SolverLimits {
            termination: Some(&expression),
            ..limits
        }))
    }
    fn static_leaves<'a, S: std::str::FromStr>(
        &self,
        construction: bool,
    ) -> Result<Vec<ScoredSearchTermination<'a, S>>, XmlTerminationError> {
        let mut leaves = Vec::new();
        let mut push = |limit| {
            leaves.push(ScoredSearchTermination::Condition(
                ScoredSearchLimit::Search(limit),
            ))
        };
        if let Some(t) = self.spent.resolve()? {
            push(SearchLimit::Time(t));
        }
        if let Some(t) = self.unimproved.resolve()? {
            push(SearchLimit::UnimprovedTime(t));
        }
        if let Some(n) = self.steps {
            push(if construction {
                SearchLimit::ConstructionSteps(n)
            } else {
                SearchLimit::Steps(n)
            });
        }
        if let Some(n) = self.unimproved_steps {
            push(SearchLimit::UnimprovedSteps(n));
        }
        if let Some(score) = self.score()? {
            leaves.push(ScoredSearchTermination::Condition(
                ScoredSearchLimit::BestScore(score),
            ));
        }
        Ok(leaves)
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct XmlPhaseTermination {
    pub kind: String,
    pub settings: Option<XmlTerminationSettings>,
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct XmlTerminationPlan {
    pub solver: Option<XmlTerminationSettings>,
    pub phases: Vec<XmlPhaseTermination>,
}
fn termination(
    n: &SolverXmlElement,
) -> Result<Option<XmlTerminationSettings>, XmlTerminationError> {
    let nodes = n
        .children
        .iter()
        .filter(|c| c.name == "termination")
        .collect::<Vec<_>>();
    if nodes.len() > 1 {
        return Err(error("duplicate termination"));
    }
    nodes
        .first()
        .map(|n| XmlTerminationSettings::from_element(n))
        .transpose()
}
impl SolverXmlConfiguration {
    /// Resolve native executable controls; unsupported options reject rather than
    /// being accepted as operational. Symbolic XML read/write remains lossless.
    pub fn native_termination_plan(&self) -> Result<XmlTerminationPlan, XmlTerminationError> {
        let root = self.root();
        Ok(XmlTerminationPlan {
            solver: termination(root)?,
            phases: root
                .children
                .iter()
                .filter(|n| {
                    matches!(
                        n.name.as_str(),
                        "constructionHeuristic"
                            | "localSearch"
                            | "customPhase"
                            | "exhaustiveSearch"
                            | "partitionedSearch"
                            | "ruinRecreate"
                    )
                })
                .map(|n| {
                    Ok(XmlPhaseTermination {
                        kind: n.name.clone(),
                        settings: termination(n)?,
                    })
                })
                .collect::<Result<_, XmlTerminationError>>()?,
        })
    }
}
fn kind<S>(p: &SolverPhase<'_, S>) -> &'static str {
    match p {
        SolverPhase::Construction(_) => "constructionHeuristic",
        SolverPhase::LocalSearch(_) => "localSearch",
        SolverPhase::Custom(_) => "customPhase",
        SolverPhase::Exact(_) => "exhaustiveSearch",
        SolverPhase::RuinRecreate(_) => "ruinRecreate",
    }
}
fn mixed_kind<S>(p: &MixedSolverPhase<'_, S>) -> &'static str {
    match p {
        MixedSolverPhase::Construction(_) => "constructionHeuristic",
        MixedSolverPhase::LocalSearch(_) => "localSearch",
        MixedSolverPhase::Custom(_) => "customPhase",
        MixedSolverPhase::Exact(_) => "exhaustiveSearch",
        MixedSolverPhase::RuinRecreate(_) => "ruinRecreate",
    }
}
impl XmlTerminationPlan {
    fn validate(&self) -> Result<(), XmlTerminationError> {
        if let Some(raw) = &self.solver {
            raw.validate()?;
        }
        for phase in &self.phases {
            if let Some(raw) = &phase.settings {
                raw.validate()?;
            }
        }
        Ok(())
    }
    fn check(&self, kinds: impl Iterator<Item = &'static str>) -> Result<(), XmlTerminationError> {
        if self.phases.is_empty() {
            return Ok(());
        }
        let kinds = kinds.collect::<Vec<_>>();
        if kinds.len() != self.phases.len()
            || kinds
                .iter()
                .zip(&self.phases)
                .any(|(kind, raw)| *kind != raw.kind)
        {
            return Err(error(
                "XML phase declarations do not match native sequence order/types",
            ));
        }
        Ok(())
    }
    /// Validate every binding before invoking run. Existing native diminished
    /// policies win; otherwise explicit XML phase tuning wins over solver tuning.
    /// Solver elapsed/stagnation counters retain existing native solve boundaries.
    pub fn with_sequence<'a, S: std::str::FromStr, R>(
        &self,
        mut sequence: SolverSequence<'a, S>,
        run: impl FnOnce(SolverSequence<'_, S>) -> R,
    ) -> Result<R, XmlTerminationError> {
        self.validate()?;
        for raw in self
            .solver
            .iter()
            .chain(self.phases.iter().filter_map(|p| p.settings.as_ref()))
        {
            raw.score::<S>()?;
        }
        self.check(sequence.phases.iter().map(kind))?;
        for (i, p) in sequence.phases.iter_mut().enumerate() {
            if let Some(raw) = self.phases.get(i).and_then(|p| p.settings.as_ref()) {
                raw.phase(p)?;
            }
            if let (Some(root), SolverPhase::LocalSearch(c)) = (&self.solver, p)
                && let Some(raw) = &root.diminished_returns
            {
                let policy = raw.resolve()?;
                c.diminished_returns.get_or_insert(policy);
            }
        }
        match &self.solver {
            Some(raw) => raw.with_limits(sequence.limits, |limits| {
                run(SolverSequence {
                    phases: sequence.phases,
                    limits,
                })
            }),
            None => Ok(run(sequence)),
        }
    }
    pub fn with_mixed_sequence<'a, S: std::str::FromStr, R>(
        &self,
        mut sequence: MixedSolverSequence<'a, S>,
        run: impl FnOnce(MixedSolverSequence<'_, S>) -> R,
    ) -> Result<R, XmlTerminationError> {
        self.validate()?;
        for raw in self
            .solver
            .iter()
            .chain(self.phases.iter().filter_map(|p| p.settings.as_ref()))
        {
            raw.score::<S>()?;
        }
        self.check(sequence.phases.iter().map(mixed_kind))?;
        for (i, p) in sequence.phases.iter_mut().enumerate() {
            if let Some(raw) = self.phases.get(i).and_then(|p| p.settings.as_ref()) {
                raw.mixed_phase(p)?;
            }
            if let (Some(root), MixedSolverPhase::LocalSearch(c)) = (&self.solver, p)
                && let Some(raw) = &root.diminished_returns
            {
                let policy = raw.resolve()?;
                c.search.diminished_returns.get_or_insert(policy);
            }
        }
        match &self.solver {
            Some(raw) => raw.with_limits(sequence.limits, |limits| {
                run(MixedSolverSequence {
                    phases: sequence.phases,
                    limits,
                })
            }),
            None => Ok(run(sequence)),
        }
    }
}

impl XmlTerminationPlan {
    fn with_phase_bindings<S: AdditiveScore + std::str::FromStr, R>(
        &self,
        kinds: &[&str],
        existing: &[PhaseTerminationBinding<'_, S>],
        run: impl FnOnce(&[PhaseTerminationBinding<'_, S>]) -> R,
    ) -> Result<R, XmlTerminationError> {
        self.validate()?;
        let mut seen = std::collections::BTreeSet::new();
        for binding in existing {
            if binding.phase() >= kinds.len() || !seen.insert(binding.phase()) {
                return Err(error(
                    "existing phase bindings require unique valid indices",
                ));
            }
        }
        let mut selected = Vec::new();
        for (i, phase) in self.phases.iter().enumerate() {
            let Some(raw) = &phase.settings else { continue };
            let tree =
                raw.compile::<S>(kinds[i] == "constructionHeuristic", Some(kinds[i]), false)?;
            if tree.is_empty() {
                continue;
            }
            if existing.iter().any(|b| b.phase() == i) {
                return Err(error(
                    "XML termination conflicts with an existing phase binding",
                ));
            }
            selected.push((i, tree));
        }
        let mut callback = Some(run);
        Ok(with_bindings(&selected, existing, &mut |bindings| {
            callback.take().unwrap()(bindings)
        }))
    }
    fn with_composed_limits<S: AdditiveScore + std::str::FromStr, R>(
        &self,
        limits: SolverLimits<'_, S>,
        run: impl FnOnce(SolverLimits<'_, S>) -> R,
    ) -> Result<R, XmlTerminationError> {
        let Some(raw) = &self.solver else {
            return Ok(run(limits));
        };
        let tree = raw.compile::<S>(false, None, false)?;
        if tree.is_empty() {
            return Ok(run(limits));
        }
        let mut callback = Some(run);
        tree.with_scored(&mut |xml| {
            let combined = limits
                .scored_termination
                .map(|old| [old.clone(), xml.clone()]);
            let expression = combined.as_ref().map_or_else(
                || xml.clone(),
                |children| ScoredSearchTermination::Any(children),
            );
            Ok(callback.take().unwrap()(SolverLimits {
                scored_termination: Some(&expression),
                ..limits.clone()
            }))
        })
    }

    /// Apply solve-wide XML AND/OR and borrow phase expressions for a solve.
    /// The closure must set `SolverExtensions::phase_terminations` to the supplied
    /// bindings before calling its solver. The expression storage stays alive only
    /// for this closure, like the sequence's borrowed global termination expression.
    /// XML phase expressions replace no native controls: caller hard caps,
    /// cancellation, callbacks and other extension fields retain their scope.
    /// A phase with an existing extension binding rejects an XML binding.
    /// Solver diminished returns remains inherited local-search tuning; solver
    /// AND with diminished returns rejects because its counters span a different scope.
    pub fn with_sequence_extensions<S: AdditiveScore + std::str::FromStr, R>(
        &self,
        mut sequence: SolverSequence<'_, S>,
        existing: &[PhaseTerminationBinding<'_, S>],
        run: impl FnOnce(SolverSequence<'_, S>, &[PhaseTerminationBinding<'_, S>]) -> R,
    ) -> Result<R, XmlTerminationError> {
        self.validate()?;
        self.check(sequence.phases.iter().map(kind))?;
        let kinds = sequence.phases.iter().map(kind).collect::<Vec<_>>();
        if let Some(root) = &self.solver {
            if root.has_solver_and_diminished(false) {
                return Err(error(
                    "solver AND cannot combine phase-local diminishedReturns with solver counters",
                ));
            }
            if let Some(dim) = root.inherited_diminished()? {
                for (i, phase) in sequence.phases.iter_mut().enumerate() {
                    if self
                        .phases
                        .get(i)
                        .and_then(|p| p.settings.as_ref())
                        .is_some_and(XmlTerminationSettings::has_diminished)
                    {
                        continue;
                    }
                    if let SolverPhase::LocalSearch(c) = phase {
                        c.diminished_returns.get_or_insert(dim.resolve()?);
                    }
                }
            }
        }
        self.with_phase_bindings(&kinds, existing, |bindings| {
            self.with_composed_limits(sequence.limits, |limits| {
                run(
                    SolverSequence {
                        phases: sequence.phases,
                        limits,
                    },
                    bindings,
                )
            })
        })?
    }

    /// Mixed-model counterpart of [`Self::with_sequence_extensions`].
    pub fn with_mixed_sequence_extensions<S: AdditiveScore + std::str::FromStr, R>(
        &self,
        mut sequence: MixedSolverSequence<'_, S>,
        existing: &[PhaseTerminationBinding<'_, S>],
        run: impl FnOnce(MixedSolverSequence<'_, S>, &[PhaseTerminationBinding<'_, S>]) -> R,
    ) -> Result<R, XmlTerminationError> {
        self.validate()?;
        self.check(sequence.phases.iter().map(mixed_kind))?;
        let kinds = sequence.phases.iter().map(mixed_kind).collect::<Vec<_>>();
        if let Some(root) = &self.solver {
            if root.has_solver_and_diminished(false) {
                return Err(error(
                    "solver AND cannot combine phase-local diminishedReturns with solver counters",
                ));
            }
            if let Some(dim) = root.inherited_diminished()? {
                for (i, phase) in sequence.phases.iter_mut().enumerate() {
                    if self
                        .phases
                        .get(i)
                        .and_then(|p| p.settings.as_ref())
                        .is_some_and(XmlTerminationSettings::has_diminished)
                    {
                        continue;
                    }
                    if let MixedSolverPhase::LocalSearch(c) = phase {
                        c.search.diminished_returns.get_or_insert(dim.resolve()?);
                    }
                }
            }
        }
        self.with_phase_bindings(&kinds, existing, |bindings| {
            self.with_composed_limits(sequence.limits, |limits| {
                run(
                    MixedSolverSequence {
                        phases: sequence.phases,
                        limits,
                    },
                    bindings,
                )
            })
        })?
    }
}

impl XmlTerminationSettings {
    fn reject_nested_projection(&self) -> Result<(), XmlTerminationError> {
        if !self.terminations.is_empty() {
            return Err(error(
                "nested termination requires with_sequence_extensions or with_mixed_sequence_extensions",
            ));
        }
        Ok(())
    }
    fn has_diminished(&self) -> bool {
        self.diminished_returns.is_some() || self.terminations.iter().any(Self::has_diminished)
    }
    fn has_solver_and_diminished(&self, parent_and: bool) -> bool {
        let and = parent_and || self.composition == Some(XmlTerminationComposition::And);
        (and && self.diminished_returns.is_some())
            || self
                .terminations
                .iter()
                .any(|c| c.has_solver_and_diminished(and))
    }
    fn inherited_diminished(&self) -> Result<Option<&XmlDiminishedReturns>, XmlTerminationError> {
        let mut found = self.diminished_returns.as_ref();
        for child in &self.terminations {
            if let Some(dim) = child.inherited_diminished()? {
                if found.is_some() {
                    return Err(error(
                        "multiple solver diminishedReturns policies cannot be inherited into one phase",
                    ));
                }
                found = Some(dim);
            }
        }
        Ok(found)
    }
    fn compile<S: AdditiveScore + std::str::FromStr>(
        &self,
        construction: bool,
        phase: Option<&str>,
        parent_and: bool,
    ) -> Result<OwnedTermination<S>, XmlTerminationError> {
        let and = self.composition == Some(XmlTerminationComposition::And);
        if phase.is_none() && self.has_solver_and_diminished(parent_and) {
            return Err(error(
                "solver AND cannot combine phase-local diminishedReturns with solver counters",
            ));
        }
        if construction && (self.unimproved.resolve()?.is_some() || self.unimproved_steps.is_some())
        {
            return Err(error("construction cannot use search stagnation"));
        }
        if self.diminished_returns.is_some() && phase.is_some_and(|p| p != "localSearch") {
            return Err(error("diminishedReturns requires a localSearch phase"));
        }
        let leaves = self
            .static_leaves::<S>(construction)?
            .into_iter()
            .map(|leaf| match leaf {
                ScoredSearchTermination::Condition(c) => c,
                _ => unreachable!(),
            })
            .collect();
        let diminished = if phase.is_some() {
            self.diminished_returns
                .as_ref()
                .map(|raw| {
                    PhaseDiminishedReturnsConfig::new(raw.resolve()?, &S::ZERO)
                        .map_err(|e| error(e.to_string()))
                })
                .transpose()?
        } else {
            None
        };
        let children = self
            .terminations
            .iter()
            .map(|c| c.compile(construction, phase, parent_and || and))
            .collect::<Result<_, _>>()?;
        Ok(OwnedTermination {
            and,
            leaves,
            diminished,
            children,
        })
    }
}

// Owned nodes contain values only. Each borrowed expression exists inside its
// callback's stack scope, including every sibling's recursively borrowed storage.
struct OwnedTermination<S> {
    and: bool,
    leaves: Vec<ScoredSearchLimit<S>>,
    diminished: Option<PhaseDiminishedReturnsConfig<S>>,
    children: Vec<Self>,
}
impl<S: Clone> OwnedTermination<S> {
    fn is_empty(&self) -> bool {
        self.leaves.is_empty()
            && self.diminished.is_none()
            && self.children.iter().all(Self::is_empty)
    }
    fn with_scored<R>(&self, run: &mut dyn FnMut(&ScoredSearchTermination<'_, S>) -> R) -> R {
        let leaves = self
            .leaves
            .iter()
            .cloned()
            .map(ScoredSearchTermination::Condition)
            .collect::<Vec<_>>();
        scored_children(&self.children, &leaves, &mut |row| {
            let expression = if self.and {
                ScoredSearchTermination::All(row)
            } else {
                ScoredSearchTermination::Any(row)
            };
            run(&expression)
        })
    }
    fn with_stateful<R>(&self, run: &mut dyn FnMut(&StatefulSearchTermination<'_, S>) -> R) -> R {
        let statics = self
            .leaves
            .iter()
            .cloned()
            .map(ScoredSearchTermination::Condition)
            .collect::<Vec<_>>();
        let mut leaves = statics
            .iter()
            .map(|leaf| StatefulSearchTermination::Condition(StatefulSearchLimit::Static(leaf)))
            .collect::<Vec<_>>();
        if let Some(dim) = &self.diminished {
            leaves.push(StatefulSearchTermination::Condition(
                StatefulSearchLimit::DiminishedReturns(dim.clone()),
            ));
        }
        stateful_children(&self.children, &leaves, &mut |row| {
            let expression = if self.and {
                StatefulSearchTermination::All(row)
            } else {
                StatefulSearchTermination::Any(row)
            };
            run(&expression)
        })
    }
}
fn scored_children<S: Clone, R>(
    children: &[OwnedTermination<S>],
    row: &[ScoredSearchTermination<'_, S>],
    run: &mut dyn FnMut(&[ScoredSearchTermination<'_, S>]) -> R,
) -> R {
    let Some((first, rest)) = children.split_first() else {
        return run(row);
    };
    if first.is_empty() {
        return scored_children(rest, row, run);
    }
    first.with_scored(&mut |child| {
        let mut next = row.to_vec();
        next.push(child.clone());
        scored_children(rest, &next, run)
    })
}
fn stateful_children<S: Clone, R>(
    children: &[OwnedTermination<S>],
    row: &[StatefulSearchTermination<'_, S>],
    run: &mut dyn FnMut(&[StatefulSearchTermination<'_, S>]) -> R,
) -> R {
    let Some((first, rest)) = children.split_first() else {
        return run(row);
    };
    if first.is_empty() {
        return stateful_children(rest, row, run);
    }
    first.with_stateful(&mut |child| {
        let mut next = row.to_vec();
        next.push(child.clone());
        stateful_children(rest, &next, run)
    })
}
fn with_bindings<S: Clone, R>(
    trees: &[(usize, OwnedTermination<S>)],
    bindings: &[PhaseTerminationBinding<'_, S>],
    run: &mut dyn FnMut(&[PhaseTerminationBinding<'_, S>]) -> R,
) -> R {
    let Some(((i, first), rest)) = trees.split_first() else {
        return run(bindings);
    };
    first.with_stateful(&mut |expression| {
        let mut next = bindings
            .iter()
            .map(|b| PhaseTerminationBinding {
                phase: b.phase,
                search: b.search,
                scored: b.scored,
                stateful: b.stateful,
            })
            .collect::<Vec<_>>();
        next.push(PhaseTerminationBinding::stateful(*i, expression));
        with_bindings(rest, &next, run)
    })
}
