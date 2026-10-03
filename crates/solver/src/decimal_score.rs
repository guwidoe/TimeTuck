//! Owned arbitrary-precision decimal score families.
//!
//! Levels compare and hash numerically, independent of trailing zeroes. Ordinary
//! arithmetic clears structural state. Exact Decimal multiply/divide factors and
//! nonnegative integer powers replace Java's floating inputs; results floor at
//! each original level's scale, clamped to zero. Large coefficients or scale gaps
//! can require proportional memory/time; these APIs have no fixed resource bound.
//!
//! ```
//! use timetuck::{Decimal, HardSoftDecimalScore};
//! let score: HardSoftDecimalScore = "0.00hard/-1.99soft".parse().unwrap();
//! let factor: Decimal = "1.2".parse().unwrap();
//! assert_eq!(score.multiply(&factor).unwrap().to_string(), "0.00hard/-2.39soft");
//! assert_eq!(score, "0hard/-1.9900soft".parse().unwrap());
//! assert!(score.is_feasible());
//! ```
use crate::{AdditiveScore, BigInt, Decimal, DecimalArithmeticError, Score};
use std::{
    cmp::Ordering,
    error::Error,
    fmt,
    hash::{Hash, Hasher},
    ops,
    str::FromStr,
};
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParseDecimalScoreError;
impl fmt::Display for ParseDecimalScoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("invalid decimal score: expected complete ordered levels")
    }
}
impl Error for ParseDecimalScoreError {}
fn zero(v: &Decimal) -> bool {
    v.coefficient() == &BigInt::ZERO
}
fn numeric_cmp(a: &[Decimal], b: &[Decimal]) -> Ordering {
    a.iter()
        .zip(b)
        .map(|(a, b)| a.numeric_cmp(b))
        .find(|o| !o.is_eq())
        .unwrap_or(Ordering::Equal)
}
macro_rules! decimal_score {
    ($name:ident,$n:expr,$hard:expr,[$($field:ident:$index:expr=>$label:literal),+],$doc:literal) => {
        #[doc=$doc]
        #[derive(Clone,Debug)]
        pub struct $name {structural:i64,levels:[Decimal;$n]}
        impl $name {
            pub const ZERO:Self=Self {structural:0,levels:[const {Decimal::ZERO};$n]};
            /// Construct a sound score, normalizing all-zero levels to ZERO.
            /// Otherwise every supplied scale is retained, including zero levels.
            pub fn new($($field:Decimal),+)->Self {Self::normalized([$($field),+])}
            fn normalized(levels:[Decimal;$n])->Self {
                if levels.iter().all(zero) {Self::ZERO} else {Self {structural:0,levels}}
            }
            /// Preserve all supplied levels/scales and the structural component.
            pub fn from_parts(structural:i64,levels:[Decimal;$n])->Self {Self {structural,levels}}
            pub fn with_structural(mut self,structural:i64)->Self {self.structural=structural;self}
            pub fn structural(&self)->i64 {self.structural}
            pub fn levels(&self)->&[Decimal;$n] {&self.levels}
            $(pub fn $field(&self)->&Decimal {&self.levels[$index]})+
            pub fn is_feasible(&self)->bool {self.structural>=0 && self.levels[..$hard].iter().all(|v|!v.numeric_cmp(&Decimal::ZERO).is_lt())}
            pub fn is_zero(&self)->bool {self==&Self::ZERO}
            fn map(&self,f:impl Fn(&Decimal)->Decimal)->Self {Self::normalized(std::array::from_fn(|i|f(&self.levels[i])))}
            fn try_map(&self,f:impl Fn(&Decimal)->Result<Decimal,DecimalArithmeticError>)->Result<Self,DecimalArithmeticError> {
                let mut levels=[const {Decimal::ZERO};$n];
                for (out,v) in levels.iter_mut().zip(&self.levels) {*out=f(v)?;}
                Ok(Self::normalized(levels))
            }
            /// Exact decimal multiplication, floored at max(original scale, 0).
            /// Arithmetic clears structural state; factors never pass through f64.
            pub fn multiply(&self,factor:&Decimal)->Result<Self,DecimalArithmeticError> {self.try_map(|v|v.multiply_floor(factor))}
            /// Exact decimal division, floored at max(original scale, 0).
            pub fn divide(&self,divisor:&Decimal)->Result<Self,DecimalArithmeticError> {self.try_map(|v|v.divide_floor(divisor))}
            /// Nonnegative integer power, floored at max(original scale, 0).
            /// Storage/time grow with the coefficient/exponent, without a fixed bound.
            pub fn power(&self,exponent:u32)->Result<Self,DecimalArithmeticError> {self.try_map(|v|v.power_floor(exponent))}
            pub fn abs(&self)->Self {self.map(|v|Decimal::new(BigInt::from(v.coefficient().magnitude().clone()),v.scale()))}
            pub fn to_short_string(&self)->String {
                let mut parts=Vec::new();
                if self.structural!=0 {parts.push(format!("{}structural",self.structural));}
                $(if !zero(&self.levels[$index]) {parts.push(format!("{}{}",self.levels[$index].to_standard_string(),$label));})+
                if parts.is_empty() {"0".into()} else {parts.join("/")}
            }
        }
        impl Default for $name {fn default()->Self {Self::ZERO}}
        impl PartialEq for $name {fn eq(&self,o:&Self)->bool {self.cmp(o).is_eq()}}
        impl Eq for $name {}
        impl PartialOrd for $name {fn partial_cmp(&self,o:&Self)->Option<Ordering> {Some(self.cmp(o))}}
        impl Ord for $name {fn cmp(&self,o:&Self)->Ordering {self.structural.cmp(&o.structural).then_with(||numeric_cmp(&self.levels,&o.levels))}}
        impl Hash for $name {fn hash<H:Hasher>(&self,h:&mut H) {self.structural.hash(h);for v in &self.levels {v.numeric_hash(h);}}}
        impl Score for $name {
            type Level=Decimal;
            const HARD_LEVEL_COUNT:usize=$hard;
            fn ordinary_levels(&self)->impl Iterator<Item=Decimal> {self.levels.iter().cloned()}
            fn structural(&self)->i64 {self.structural}
            fn hard_cmp(&self,o:&Self)->Ordering {numeric_cmp(&self.levels[..$hard],&o.levels[..$hard])}
            fn is_feasible(&self)->bool {self.is_feasible()}
        }
        impl AdditiveScore for $name {
            const ZERO:Self=Self::ZERO;
            fn with_structural(self,s:i64)->Self {self.with_structural(s)}
        }
        impl ops::Add for $name {type Output=Self;fn add(self,o:Self)->Self {Self::normalized(std::array::from_fn(|i|self.levels[i].add(&o.levels[i])))}}
        impl ops::Sub for $name {type Output=Self;fn sub(self,o:Self)->Self {Self::normalized(std::array::from_fn(|i|self.levels[i].subtract(&o.levels[i])))}}
        impl ops::Neg for $name {type Output=Self;fn neg(self)->Self {if self.is_zero() {self} else {Self::ZERO-self}}}
        impl fmt::Display for $name {
            fn fmt(&self,f:&mut fmt::Formatter<'_>)->fmt::Result {
                if self.structural<0 {write!(f,"{}structural/",self.structural)?;}
                for (i,(v,label)) in self.levels.iter().zip([$($label),+]).enumerate() {
                    if i!=0 {f.write_str("/")?;}write!(f,"{}{label}",v.to_standard_string())?;
                }
                Ok(())
            }
        }
        impl FromStr for $name {
            type Err=ParseDecimalScoreError;
            fn from_str(text:&str)->Result<Self,Self::Err> {
                let (structural,body)=if let Some((s,b))=text.split_once("structural/") {(Some(crate::score::number(s).map_err(|_|ParseDecimalScoreError)?),b)} else {(None,text)};
                let mut parts=body.split('/'); let mut levels=[const {Decimal::ZERO};$n];
                for (out,label) in levels.iter_mut().zip([$($label),+]) {
                    *out=parts.next().and_then(|p|p.strip_suffix(label)).ok_or(ParseDecimalScoreError)?.parse().map_err(|_|ParseDecimalScoreError)?;
                }
                if parts.next().is_some() {return Err(ParseDecimalScoreError);}
                Ok(match structural {Some(s)=>Self::from_parts(s,levels),None=>Self::normalized(levels)})
            }
        }
    }
}
decimal_score!(SimpleDecimalScore,1,0,[score:0=>""],"One arbitrary-precision decimal level and structural state.");
decimal_score!(HardSoftDecimalScore,2,1,[hard:0=>"hard",soft:1=>"soft"],"One hard and one soft arbitrary-precision decimal level.");
decimal_score!(HardMediumSoftDecimalScore,3,1,[hard:0=>"hard",medium:1=>"medium",soft:2=>"soft"],"Hard, medium and soft arbitrary-precision decimal levels.");
impl HardSoftDecimalScore {
    pub fn of_hard(v: Decimal) -> Self {
        Self::new(v, Decimal::ZERO)
    }
    pub fn of_soft(v: Decimal) -> Self {
        Self::new(Decimal::ZERO, v)
    }
}
impl HardMediumSoftDecimalScore {
    pub fn of_hard(v: Decimal) -> Self {
        Self::new(v, Decimal::ZERO, Decimal::ZERO)
    }
    pub fn of_medium(v: Decimal) -> Self {
        Self::new(Decimal::ZERO, v, Decimal::ZERO)
    }
    pub fn of_soft(v: Decimal) -> Self {
        Self::new(Decimal::ZERO, Decimal::ZERO, v)
    }
}
