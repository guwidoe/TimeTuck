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
#[derive(Clone, Debug, Default, PartialEq)]
pub struct XmlTerminationSettings {
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
                "bestScoreLimit",
                "stepCountLimit",
                "unimprovedStepCountLimit",
                "diminishedReturns",
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
        Ok(Self {
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
            best_score_limit: scalar(n, "bestScoreLimit")?.map(str::to_owned),
            steps: number(n, "stepCountLimit")?,
            unimproved_steps: number(n, "unimprovedStepCountLimit")?,
            diminished_returns,
        })
    }
    fn validate(&self) -> Result<(), XmlTerminationError> {
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
    fn phase<S: std::str::FromStr>(
        &self,
        p: &mut SolverPhase<'_, S>,
    ) -> Result<(), XmlTerminationError> {
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
