//! Exact score strings and typed JSON envelopes for persisted solver results.
use crate::{
    BendableDecimalScore, BendableScore, ConstraintWeightOverrides, Decimal,
    HardMediumSoftDecimalScore, HardMediumSoftScore, HardSoftDecimalScore, HardSoftScore,
    JsonValue, Score, SimpleDecimalScore, SimpleScore,
};
use std::{error::Error, fmt, str::FromStr};

/// The native score family and (for bendable scores) compile-time dimensions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScoreJsonFormat {
    Simple,
    HardSoft,
    HardMediumSoft,
    Bendable { hard: usize, soft: usize },
    SimpleDecimal,
    HardSoftDecimal,
    HardMediumSoftDecimal,
    BendableDecimal { hard: usize, soft: usize },
}
impl ScoreJsonFormat {
    pub fn name(self) -> &'static str {
        match self {
            Self::Simple => "simple",
            Self::HardSoft => "hard_soft",
            Self::HardMediumSoft => "hard_medium_soft",
            Self::Bendable { .. } => "bendable",
            Self::SimpleDecimal => "simple_decimal",
            Self::HardSoftDecimal => "hard_soft_decimal",
            Self::HardMediumSoftDecimal => "hard_medium_soft_decimal",
            Self::BendableDecimal { .. } => "bendable_decimal",
        }
    }
    pub fn dimensions(self) -> Option<(usize, usize)> {
        match self {
            Self::Bendable { hard, soft } | Self::BendableDecimal { hard, soft } => {
                Some((hard, soft))
            }
            _ => None,
        }
    }
    fn validate(self, text: &str) -> Result<(), ScoreJsonError> {
        let valid = match self {
            Self::Simple => text.parse::<SimpleScore>().is_ok(),
            Self::HardSoft => text.parse::<HardSoftScore>().is_ok(),
            Self::HardMediumSoft => text.parse::<HardMediumSoftScore>().is_ok(),
            Self::SimpleDecimal => text.parse::<SimpleDecimalScore>().is_ok(),
            Self::HardSoftDecimal => text.parse::<HardSoftDecimalScore>().is_ok(),
            Self::HardMediumSoftDecimal => text.parse::<HardMediumSoftDecimalScore>().is_ok(),
            Self::Bendable { hard, soft } => valid_bendable(text, hard, soft, false),
            Self::BendableDecimal { hard, soft } => valid_bendable(text, hard, soft, true),
        };
        if valid {
            Ok(())
        } else {
            Err(ScoreJsonError::InvalidScore)
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ScoreJsonError {
    ExpectedStringOrNull,
    ExpectedObject,
    InvalidScore,
    MissingField(&'static str),
    UnexpectedField(String),
    UnknownType(String),
    InvalidDimensions,
    FormatMismatch {
        expected: ScoreJsonFormat,
        actual: ScoreJsonFormat,
    },
    InvalidWeight {
        constraint: String,
    },
}
impl fmt::Display for ScoreJsonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "score JSON: {self:?}")
    }
}
impl Error for ScoreJsonError {}
mod sealed {
    pub trait Sealed {}
}
/// JSON support for the eight built-in score families, including every bendable
/// dimension. Ordinary values remain strings: integer width and decimal precision
/// never pass through JSON floating-point numbers.
pub trait JsonScore: Score + fmt::Display + FromStr + sealed::Sealed {
    const JSON_FORMAT: ScoreJsonFormat;
}
macro_rules! fixed {
    ($($ty:ty => $kind:ident),+ $(,)?) => { $(
        impl sealed::Sealed for $ty {}
        impl JsonScore for $ty { const JSON_FORMAT: ScoreJsonFormat = ScoreJsonFormat::$kind; }
    )+ };
}
fixed!(SimpleScore => Simple, HardSoftScore => HardSoft, HardMediumSoftScore => HardMediumSoft,
    SimpleDecimalScore => SimpleDecimal, HardSoftDecimalScore => HardSoftDecimal,
    HardMediumSoftDecimalScore => HardMediumSoftDecimal);
impl<const H: usize, const S: usize> sealed::Sealed for BendableScore<H, S> {}
impl<const H: usize, const S: usize> JsonScore for BendableScore<H, S> {
    const JSON_FORMAT: ScoreJsonFormat = ScoreJsonFormat::Bendable { hard: H, soft: S };
}
impl<const H: usize, const S: usize> sealed::Sealed for BendableDecimalScore<H, S> {}
impl<const H: usize, const S: usize> JsonScore for BendableDecimalScore<H, S> {
    const JSON_FORMAT: ScoreJsonFormat = ScoreJsonFormat::BendableDecimal { hard: H, soft: S };
}
fn score_text<S: JsonScore>(score: &S) -> String {
    // Display intentionally omits positive structural state. Preserve it at this
    // persistence boundary; all native parsers already accept the explicit prefix.
    if score.structural() > 0 {
        format!("{}structural/{score}", score.structural())
    } else {
        score.to_string()
    }
}
/// Encode a nullable score as a JSON string or null. Positive structural state,
/// unlike native Display, is included explicitly so persistence is lossless.
pub fn score_to_json<S: JsonScore>(score: Option<&S>) -> JsonValue {
    score.map_or(JsonValue::Null, |score| {
        JsonValue::String(score_text(score))
    })
}
/// Decode a nullable score with the shape chosen by the caller's Rust type.
/// Numbers, arrays and objects are rejected, including numeric zero.
pub fn score_from_json<S: JsonScore>(value: &JsonValue) -> Result<Option<S>, ScoreJsonError> {
    match value {
        JsonValue::Null => Ok(None),
        JsonValue::String(text) => text
            .parse()
            .map(Some)
            .map_err(|_| ScoreJsonError::InvalidScore),
        _ => Err(ScoreJsonError::ExpectedStringOrNull),
    }
}
/// A validated, owned score with a native type tag. This is a transport value,
/// not an arithmetic score: decode it into a concrete score before solving.
/// Tags use snake_case names from `ScoreJsonFormat::name`; bendable envelopes also
/// require `hard_levels` and `soft_levels`. Unknown properties are rejected.
/// It preserves the accepted score text; typed decoding uses native score parsers.
///
/// ```
/// use timetuck::*;
/// let score = BendableScore::new([-1, 20], [30]).with_structural(-2);
/// let json = ScoreEnvelope::capture(&score).to_json();
/// let envelope = ScoreEnvelope::from_json(&json)?;
/// assert_eq!(envelope.decode::<BendableScore<2,1>>()?, score);
/// assert!(envelope.decode::<BendableScore<1,2>>().is_err());
/// # Ok::<(), ScoreJsonError>(())
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScoreEnvelope {
    format: ScoreJsonFormat,
    text: String,
}
impl ScoreEnvelope {
    pub fn capture<S: JsonScore>(score: &S) -> Self {
        Self {
            format: S::JSON_FORMAT,
            text: score_text(score),
        }
    }
    pub fn format(&self) -> ScoreJsonFormat {
        self.format
    }
    pub fn text(&self) -> &str {
        &self.text
    }
    pub fn decode<S: JsonScore>(&self) -> Result<S, ScoreJsonError> {
        if self.format != S::JSON_FORMAT {
            return Err(ScoreJsonError::FormatMismatch {
                expected: S::JSON_FORMAT,
                actual: self.format,
            });
        }
        self.text.parse().map_err(|_| ScoreJsonError::InvalidScore)
    }
    pub fn to_json(&self) -> JsonValue {
        let mut fields = serde_json::Map::new();
        fields.insert("type".into(), self.format.name().into());
        fields.insert("score".into(), self.text.clone().into());
        if let Some((hard, soft)) = self.format.dimensions() {
            fields.insert("hard_levels".into(), hard.into());
            fields.insert("soft_levels".into(), soft.into());
        }
        fields.into()
    }
    pub fn from_json(value: &JsonValue) -> Result<Self, ScoreJsonError> {
        let fields = value.as_object().ok_or(ScoreJsonError::ExpectedObject)?;
        let kind = fields
            .get("type")
            .and_then(JsonValue::as_str)
            .ok_or(ScoreJsonError::MissingField("type"))?;
        let text = fields
            .get("score")
            .and_then(JsonValue::as_str)
            .ok_or(ScoreJsonError::MissingField("score"))?;
        let dim = |key| {
            fields
                .get(key)
                .and_then(JsonValue::as_u64)
                .and_then(|v| usize::try_from(v).ok())
                .ok_or(ScoreJsonError::InvalidDimensions)
        };
        let format = match kind {
            "simple" => ScoreJsonFormat::Simple,
            "hard_soft" => ScoreJsonFormat::HardSoft,
            "hard_medium_soft" => ScoreJsonFormat::HardMediumSoft,
            "simple_decimal" => ScoreJsonFormat::SimpleDecimal,
            "hard_soft_decimal" => ScoreJsonFormat::HardSoftDecimal,
            "hard_medium_soft_decimal" => ScoreJsonFormat::HardMediumSoftDecimal,
            "bendable" => ScoreJsonFormat::Bendable {
                hard: dim("hard_levels")?,
                soft: dim("soft_levels")?,
            },
            "bendable_decimal" => ScoreJsonFormat::BendableDecimal {
                hard: dim("hard_levels")?,
                soft: dim("soft_levels")?,
            },
            _ => return Err(ScoreJsonError::UnknownType(kind.into())),
        };
        for key in fields.keys() {
            if key != "type"
                && key != "score"
                && !(format.dimensions().is_some()
                    && (key == "hard_levels" || key == "soft_levels"))
            {
                return Err(ScoreJsonError::UnexpectedField(key.clone()));
            }
        }
        format.validate(text)?;
        Ok(Self {
            format,
            text: text.into(),
        })
    }
}
// Validate runtime dimensions without allocating attacker-specified vectors.
// Fixed-shape decoding remains delegated to the existing native score parsers.
fn valid_bendable(text: &str, hard: usize, soft: usize, decimal: bool) -> bool {
    let body = if let Some((prefix, body)) = text.split_once("structural/") {
        if crate::score::number(prefix).is_err() {
            return false;
        }
        body
    } else {
        text
    };
    let Some((h, s)) = body.split_once("hard/") else {
        return false;
    };
    let Some(s) = s.strip_suffix("soft") else {
        return false;
    };
    [(h, hard), (s, soft)].into_iter().all(|(group, count)| {
        let Some(body) = group.strip_prefix('[').and_then(|v| v.strip_suffix(']')) else {
            return false;
        };
        if body.is_empty() {
            return count == 0;
        }
        let mut seen = 0_usize;
        for level in body.split('/') {
            if seen == count {
                return false;
            }
            if if decimal {
                level.parse::<Decimal>().is_err()
            } else {
                crate::score::number(level).is_err()
            } {
                return false;
            }
            seen += 1;
        }
        seen == count
    })
}
/// Serialize the constraint-name map deterministically with plain score strings.
/// An explicit zero remains present; missing weights continue to mean defaults.
pub fn weight_overrides_to_json<S: JsonScore>(weights: &ConstraintWeightOverrides<S>) -> JsonValue {
    weights
        .iter()
        .map(|(name, score)| (name.to_owned(), score_to_json(Some(score))))
        .collect()
}
/// Decode every weight before returning a new map. Null weights are invalid;
/// errors name the offending constraint and do not expose a partially filled map.
pub fn weight_overrides_from_json<S: JsonScore>(
    value: &JsonValue,
) -> Result<ConstraintWeightOverrides<S>, ScoreJsonError> {
    value
        .as_object()
        .ok_or(ScoreJsonError::ExpectedObject)?
        .iter()
        .map(|(name, value)| {
            let score = score_from_json(value).ok().flatten().ok_or_else(|| {
                ScoreJsonError::InvalidWeight {
                    constraint: name.clone(),
                }
            })?;
            Ok((name.clone(), score))
        })
        .collect()
}
