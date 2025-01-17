use std::fmt::Display;

use chrono::{DateTime, Datelike, Duration, NaiveDate, SubsecRound, Timelike, Utc};
use ed25519_dalek::{PublicKey, Signature};
use hashbrown::HashMap;
use locutus_stdlib::prelude::*;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use strum::Display;

type Assignment = ed25519_dalek::PublicKey;

/// Contracts making use of the allocation must implement a type with this trait that allows
/// extracting the criteria for the given contract.
pub trait TokenAllocation: DeserializeOwned {
    fn get_criteria(&self) -> AllocationCriteria;
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, Display)]
#[strum(serialize_all = "lowercase")]
#[repr(u8)]
pub enum Tier {
    Min1,
    Min5,
    Min10,
    Min30,
    Hour1,
    Hour3,
    Hour6,
    Hour12,
    Day1,
    Day7,
    Day15,
    Day30,
    Day90,
    Day180,
    Day365,
}

impl Tier {
    pub fn is_valid_slot(&self, dt: DateTime<Utc>) -> bool {
        match self {
            Tier::Min1 => {
                let vns = dt.nanosecond() == 0;
                let vs = dt.second() == 0;
                vns && vs
            }
            Tier::Min5 => Self::check_is_correct_minute(dt, 5),
            Tier::Min10 => Self::check_is_correct_minute(dt, 10),
            Tier::Min30 => Self::check_is_correct_minute(dt, 30),
            Tier::Hour1 => {
                let vns = dt.nanosecond() == 0;
                let vs = dt.second() == 0;
                let vm = dt.minute() == 0;
                vns && vs && vm
            }
            Tier::Hour3 => Self::check_is_correct_hour(dt, 3),
            Tier::Hour6 => Self::check_is_correct_hour(dt, 6),
            Tier::Hour12 => Self::check_is_correct_hour(dt, 12),
            Tier::Day1 => {
                let vns = dt.nanosecond() == 0;
                let vs = dt.second() == 0;
                let vm = dt.minute() == 0;
                let vh = dt.hour() == 0;
                vns && vs && vm && vh
            }
            Tier::Day7 => Self::check_is_correct_day(dt, 7),
            Tier::Day15 => Self::check_is_correct_day(dt, 15),
            Tier::Day30 => Self::check_is_correct_day(dt, 30),
            Tier::Day90 => Self::check_is_correct_day(dt, 90),
            Tier::Day180 => Self::check_is_correct_day(dt, 180),
            Tier::Day365 => Self::check_is_correct_day(dt, 365),
        }
    }

    fn check_is_correct_minute(dt: DateTime<Utc>, base_min: u32) -> bool {
        dt.second() == 0 && dt.nanosecond() == 0 && dt.minute() % base_min == 0
    }

    fn check_is_correct_hour(dt: DateTime<Utc>, base_hour: u32) -> bool {
        dt.minute() == 0 && dt.second() == 0 && dt.nanosecond() == 0 && dt.hour() % base_hour == 0
    }

    fn check_is_correct_day(dt: DateTime<Utc>, base_day: i64) -> bool {
        let year = get_date(dt.year() - 1, 12, 31);
        let delta = dt - year;
        dt.hour() == 0
            && dt.minute() == 0
            && dt.second() == 0
            && dt.nanosecond() == 0
            && delta.num_days() % base_day == 0
    }

    pub fn tier_duration(&self) -> std::time::Duration {
        match self {
            Tier::Min1 => Duration::minutes(1).to_std().unwrap(),
            Tier::Min5 => Duration::minutes(5).to_std().unwrap(),
            Tier::Min10 => Duration::minutes(10).to_std().unwrap(),
            Tier::Min30 => Duration::minutes(30).to_std().unwrap(),
            Tier::Hour1 => Duration::hours(1).to_std().unwrap(),
            Tier::Hour3 => Duration::hours(3).to_std().unwrap(),
            Tier::Hour6 => Duration::hours(6).to_std().unwrap(),
            Tier::Hour12 => Duration::hours(12).to_std().unwrap(),
            Tier::Day1 => Duration::days(1).to_std().unwrap(),
            Tier::Day7 => Duration::days(7).to_std().unwrap(),
            Tier::Day15 => Duration::days(15).to_std().unwrap(),
            Tier::Day30 => Duration::days(30).to_std().unwrap(),
            Tier::Day90 => Duration::days(90).to_std().unwrap(),
            Tier::Day180 => Duration::days(180).to_std().unwrap(),
            Tier::Day365 => Duration::days(365).to_std().unwrap(),
        }
    }

    /// Normalized the datetime to be the next valid date from the provided one compatible with the tier.
    ///
    /// The base reference datetime used for normalization for day tiers, is from the first day of the year (Gregorian calendar).
    /// For the hour tiers, the first hour of the day; and for minute tiers, the first minute of the hour.
    pub fn normalize_to_next(&self, mut time: DateTime<Utc>) -> DateTime<Utc> {
        match self {
            Tier::Min1 => {
                let is_rounded = time.hour() == 0 && time.second() == 0 && time.nanosecond() == 0;
                if !is_rounded {
                    let duration = chrono::Duration::from_std(self.tier_duration()).unwrap();
                    time = time.with_second(0).unwrap();
                    time = time.trunc_subsecs(0);
                    time += duration;
                }
                time
            }
            Tier::Min5 => self.normalize_to_next_minute(time, 5),
            Tier::Min10 => self.normalize_to_next_minute(time, 10),
            Tier::Min30 => self.normalize_to_next_minute(time, 15),
            Tier::Hour1 => {
                let is_rounded = time.hour() == 0
                    && time.minute() == 0
                    && time.second() == 0
                    && time.nanosecond() == 0;
                if !is_rounded {
                    let duration = chrono::Duration::from_std(self.tier_duration()).unwrap();
                    time = time.with_second(0).unwrap().with_minute(0).unwrap();
                    time = time.trunc_subsecs(0);
                    time += duration;
                }
                time
            }
            Tier::Hour3 => self.normalize_to_next_hour(time, 3),
            Tier::Hour6 => self.normalize_to_next_hour(time, 6),
            Tier::Hour12 => self.normalize_to_next_hour(time, 12),
            Tier::Day1 => {
                let is_rounded = time.hour() == 0
                    && time.minute() == 0
                    && time.second() == 0
                    && time.nanosecond() == 0;
                if !is_rounded {
                    let duration = chrono::Duration::from_std(self.tier_duration()).unwrap();
                    time = time.with_second(0).unwrap().with_minute(0).unwrap();
                    time = time.trunc_subsecs(0);
                    time += duration;
                }
                time
            }
            Tier::Day7 => self.normalize_to_next_day(time, 7),
            Tier::Day15 => self.normalize_to_next_day(time, 15),
            Tier::Day30 => self.normalize_to_next_day(time, 30),
            Tier::Day90 => self.normalize_to_next_day(time, 90),
            Tier::Day180 => self.normalize_to_next_day(time, 180),
            Tier::Day365 => self.normalize_to_next_day(time, 365),
        }
    }

    fn normalize_to_next_minute(&self, mut time: DateTime<Utc>, base_minute: u32) -> DateTime<Utc> {
        let is_rounded =
            time.minute() % base_minute == 0 && time.second() == 0 && time.nanosecond() == 0;
        if !is_rounded {
            time = time.with_second(0).unwrap();
            time = time.trunc_subsecs(0);
            let minutes_in_time = time.minute();
            let remainder_minutes = minutes_in_time % base_minute;
            if remainder_minutes != 0 {
                let duration = chrono::Duration::from_std(self.tier_duration()).unwrap();
                time = time.with_minute(time.minute() - remainder_minutes).unwrap();
                time += duration;
            }
        }
        time
    }

    fn normalize_to_next_hour(&self, mut time: DateTime<Utc>, base_hour: u32) -> DateTime<Utc> {
        let is_rounded = time.hour() % base_hour == 0
            && time.minute() == 0
            && time.second() == 0
            && time.nanosecond() == 0;
        if !is_rounded {
            time = time.with_second(0).unwrap().with_minute(0).unwrap();
            time = time.trunc_subsecs(0);
            let hours_in_time = time.hour();
            let remainder_hours = hours_in_time % base_hour;
            if remainder_hours != 0 {
                let duration = chrono::Duration::from_std(self.tier_duration()).unwrap();
                time = time.with_hour(time.hour() - remainder_hours).unwrap();
                time += duration;
            }
        }
        time
    }

    fn normalize_to_next_day(&self, mut time: DateTime<Utc>, base_day: i64) -> DateTime<Utc> {
        let year = get_date(time.year() - 1, 12, 31);
        let delta = time - year;
        let is_rounded = time.hour() == 0
            && time.minute() == 0
            && time.second() == 0
            && time.nanosecond() == 0
            && delta.num_days() % base_day == 0;
        if !is_rounded {
            time = time.with_second(0).unwrap().with_minute(0).unwrap();
            time = time.trunc_subsecs(0);
            let days_in_time = delta.num_days();
            let remainder_days = (days_in_time % base_day) as u32;
            if remainder_days != 0 {
                let duration = chrono::Duration::from_std(self.tier_duration()).unwrap();
                time = time.with_day(time.day() - remainder_days).unwrap();
                time += duration;
            }
        }
        time
    }
}

fn get_date(y: i32, m: u32, d: u32) -> DateTime<Utc> {
    let naive = NaiveDate::from_ymd_opt(y, m, d)
        .unwrap()
        .and_hms_opt(0, 0, 0)
        .unwrap();
    DateTime::<Utc>::from_utc(naive, Utc)
}

#[cfg(test)]
mod tier_tests {
    use super::*;

    #[test]
    fn is_correct_minute() {
        let day7_tier = Tier::Day7;
        assert!(day7_tier.is_valid_slot(get_date(2023, 1, 7)));
        assert!(!day7_tier.is_valid_slot(get_date(2023, 1, 8)));

        let day30_tier = Tier::Day30;
        assert!(day30_tier.is_valid_slot(get_date(2023, 1, 30)));
        assert!(day30_tier.is_valid_slot(get_date(2023, 3, 1)));
        assert!(!day30_tier.is_valid_slot(get_date(2023, 3, 30)));
    }

    #[test]
    fn is_correct_hour() {
        let hour3_tier = Tier::Hour3;
        assert!(hour3_tier.is_valid_slot(get_date(2023, 1, 7).with_hour(6).unwrap()));
        assert!(!hour3_tier.is_valid_slot(get_date(2023, 1, 8).with_hour(7).unwrap()));

        let hour12_tier = Tier::Hour12;
        assert!(hour12_tier.is_valid_slot(get_date(2023, 1, 30).with_hour(12).unwrap()));
        assert!(hour12_tier.is_valid_slot(get_date(2023, 3, 1)));
        assert!(!hour12_tier.is_valid_slot(get_date(2023, 3, 30).with_hour(13).unwrap()));
    }

    #[test]
    fn is_correct_day() {
        let day7_tier = Tier::Day7;
        assert!(day7_tier.is_valid_slot(get_date(2023, 1, 7)));
        assert!(!day7_tier.is_valid_slot(get_date(2023, 1, 8)));

        let day30_tier = Tier::Day30;
        assert!(day30_tier.is_valid_slot(get_date(2023, 1, 30)));
        assert!(day30_tier.is_valid_slot(get_date(2023, 3, 1)));
        assert!(!day30_tier.is_valid_slot(get_date(2023, 3, 30)));
    }

    #[test]
    fn minute_tier_normalization() {
        let min5_tier = Tier::Min5;
        let min5_normalized =
            min5_tier.normalize_to_next(get_date(2023, 1, 1).with_minute(37).unwrap());
        assert_eq!(
            min5_normalized,
            get_date(2023, 1, 1).with_minute(40).unwrap()
        );
        let min5_normalized =
            min5_tier.normalize_to_next(get_date(2023, 1, 1).with_minute(8).unwrap());
        assert_eq!(
            min5_normalized,
            get_date(2023, 1, 1).with_minute(10).unwrap()
        );

        let min10_tier = Tier::Min10;
        let min10_normalized =
            min10_tier.normalize_to_next(get_date(2023, 1, 1).with_minute(22).unwrap());
        assert_eq!(
            min10_normalized,
            get_date(2023, 1, 1).with_minute(30).unwrap()
        );
        let min10_tier = Tier::Min10;
        let min10_normalized =
            min10_tier.normalize_to_next(get_date(2023, 1, 1).with_minute(38).unwrap());
        assert_eq!(
            min10_normalized,
            get_date(2023, 1, 1).with_minute(40).unwrap()
        );
    }

    #[test]
    fn hour_tier_normalization() {
        let hour6_tier = Tier::Hour6;
        let hour6_normalized =
            hour6_tier.normalize_to_next(get_date(2023, 1, 1).with_hour(4).unwrap());
        assert_eq!(hour6_normalized, get_date(2023, 1, 1).with_hour(6).unwrap());
        let hour6_normalized =
            hour6_tier.normalize_to_next(get_date(2023, 1, 1).with_hour(17).unwrap());
        assert_eq!(
            hour6_normalized,
            get_date(2023, 1, 1).with_hour(18).unwrap()
        );

        let hour12_tier = Tier::Hour12;
        let hour12_normalized =
            hour12_tier.normalize_to_next(get_date(2023, 1, 1).with_hour(4).unwrap());
        assert_eq!(
            hour12_normalized,
            get_date(2023, 1, 1).with_hour(12).unwrap()
        );
        let hour12_normalized =
            hour12_tier.normalize_to_next(get_date(2023, 1, 1).with_hour(17).unwrap());
        assert_eq!(hour12_normalized, get_date(2023, 1, 2));
    }

    #[test]
    fn day_tier_normalization() {
        let day7_tier = Tier::Day7;
        let day7_normalized = day7_tier.normalize_to_next(get_date(2023, 1, 17));
        assert_eq!(day7_normalized, get_date(2023, 1, 21));
        let day15_normalized = day7_tier.normalize_to_next(get_date(2023, 1, 31));
        assert_eq!(day15_normalized, get_date(2023, 2, 4));

        let day15_tier = Tier::Day15;
        let day15_normalized = day15_tier.normalize_to_next(get_date(2023, 1, 17));
        assert_eq!(day15_normalized, get_date(2023, 1, 30));
        let day15_normalized = day15_tier.normalize_to_next(get_date(2023, 1, 31));
        assert_eq!(day15_normalized, get_date(2023, 2, 14));
    }
}

#[non_exhaustive]
#[derive(Serialize, Deserialize)]
pub struct TokenParameters {
    pub generator_public_key: PublicKey,
}

impl TryFrom<Parameters<'_>> for TokenParameters {
    type Error = ContractError;
    fn try_from(params: Parameters<'_>) -> Result<Self, Self::Error> {
        let this = bincode::deserialize_from(params.as_ref())
            .map_err(|err| ContractError::Deser(format!("{err}")))?;
        Ok(this)
    }
}

#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct AllocationError(Box<AllocationErrorInner>);

impl AllocationError {
    pub fn invalid_assignment(assignment: TokenAssignment) -> Self {
        Self(Box::new(AllocationErrorInner::InvalidAssignment(
            assignment,
        )))
    }

    pub fn allocated_slot(assignment: &TokenAssignment) -> Self {
        Self(Box::new(AllocationErrorInner::AllocatedSlot {
            tier: assignment.tier,
            slot: assignment.time_slot,
        }))
    }
}

impl From<AllocationErrorInner> for AllocationError {
    fn from(value: AllocationErrorInner) -> Self {
        Self(Box::new(value))
    }
}

#[derive(Debug, thiserror::Error)]
#[allow(clippy::large_enum_variant)]
enum AllocationErrorInner {
    #[error("the following slot for {tier} has already been allocated: {slot}")]
    AllocatedSlot { tier: Tier, slot: DateTime<Utc> },
    #[error("the max age allowed is 730 days")]
    IncorrectMaxAge,
    #[error("the following assignment is incorrect: {0}")]
    InvalidAssignment(TokenAssignment),
}

#[non_exhaustive]
#[derive(Debug, Serialize, Deserialize)]
pub struct AllocationCriteria {
    pub frequency: Tier,
    /// Maximum age of the allocated token.
    pub max_age: std::time::Duration,
    pub contract: ContractInstanceId,
}

impl AllocationCriteria {
    pub fn new(
        frequency: Tier,
        max_age: std::time::Duration,
        contract: ContractInstanceId,
    ) -> Result<Self, AllocationError> {
        if max_age <= std::time::Duration::from_secs(3600 * 24 * 365 * 2) {
            Ok(Self {
                frequency,
                max_age,
                contract,
            })
        } else {
            Err(AllocationErrorInner::IncorrectMaxAge.into())
        }
    }
}

#[non_exhaustive]
#[derive(Debug, Serialize, Deserialize)]
pub struct TokenAllocationRecord {
    /// A list of issued tokens.
    ///
    /// This is categorized by tiers and then sorted by time slot.
    tokens_by_tier: HashMap<Tier, Vec<TokenAssignment>>,
}

impl TokenAllocationRecord {
    pub fn get_tier(&self, tier: &Tier) -> Option<&[TokenAssignment]> {
        self.tokens_by_tier.get(tier).map(|t| t.as_slice())
    }

    pub fn get_mut_tier(&mut self, tier: &Tier) -> Option<&mut Vec<TokenAssignment>> {
        self.tokens_by_tier.get_mut(tier)
    }

    pub fn new(mut tokens: HashMap<Tier, Vec<TokenAssignment>>) -> Self {
        for (_, assignments) in &mut tokens {
            assignments.sort_unstable();
        }
        Self {
            tokens_by_tier: tokens,
        }
    }

    pub fn insert(&mut self, tier: Tier, assignments: Vec<TokenAssignment>) {
        self.tokens_by_tier.insert(tier, assignments);
    }

    pub fn summarize(&self) -> TokenAllocationSummary {
        let mut by_tier = HashMap::with_capacity(self.tokens_by_tier.len());
        for (tier, assignments) in &self.tokens_by_tier {
            let mut assignments_ts = Vec::with_capacity(assignments.len());
            for a in assignments {
                let ts = a.time_slot.timestamp();
                assignments_ts.push(ts);
            }
            by_tier.insert(*tier, assignments_ts);
        }
        TokenAllocationSummary(by_tier)
    }

    pub fn delta(&self, summary: &TokenAllocationSummary) -> TokenAllocationRecord {
        let mut delta = HashMap::new();
        for (tier, summary_assignments) in &summary.0 {
            let mut missing = vec![];
            if let Some(assigned) = self.tokens_by_tier.get(tier) {
                for a in assigned {
                    let ts = a.time_slot.timestamp();
                    if summary_assignments.binary_search(&ts).is_err() {
                        missing.push(a.clone());
                    }
                }
                delta.insert(*tier, missing);
            }
        }
        TokenAllocationRecord {
            tokens_by_tier: delta,
        }
    }

    pub fn assignment_exists(&self, record: &TokenAssignment) -> bool {
        let Some(assignments) = self.tokens_by_tier.get(&record.tier) else { return false };
        let Ok(idx) = assignments.binary_search_by(|t| t.time_slot.cmp(&record.time_slot)) else { return false };
        let assignment = &assignments[idx];
        assignment == record
    }
}

impl<'a> IntoIterator for &'a TokenAllocationRecord {
    type Item = (&'a Tier, &'a Vec<TokenAssignment>);

    type IntoIter = hashbrown::hash_map::Iter<'a, Tier, Vec<TokenAssignment>>;

    fn into_iter(self) -> Self::IntoIter {
        self.tokens_by_tier.iter()
    }
}

impl IntoIterator for TokenAllocationRecord {
    type Item = (Tier, Vec<TokenAssignment>);

    type IntoIter = hashbrown::hash_map::IntoIter<Tier, Vec<TokenAssignment>>;

    fn into_iter(self) -> Self::IntoIter {
        self.tokens_by_tier.into_iter()
    }
}

impl TryFrom<State<'_>> for TokenAllocationRecord {
    type Error = ContractError;

    fn try_from(state: State<'_>) -> Result<Self, Self::Error> {
        let this = bincode::deserialize_from(state.as_ref())
            .map_err(|err| ContractError::Deser(format!("{err}")))?;
        Ok(this)
    }
}

impl TryFrom<TokenAllocationRecord> for State<'static> {
    type Error = ContractError;

    fn try_from(state: TokenAllocationRecord) -> Result<Self, Self::Error> {
        let serialized =
            bincode::serialize(&state).map_err(|err| ContractError::Deser(format!("{err}")))?;
        Ok(State::from(serialized))
    }
}

impl TryFrom<TokenAllocationRecord> for StateDelta<'static> {
    type Error = ContractError;

    fn try_from(state: TokenAllocationRecord) -> Result<Self, Self::Error> {
        let serialized =
            bincode::serialize(&state).map_err(|err| ContractError::Deser(format!("{err}")))?;
        Ok(StateDelta::from(serialized))
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenAllocationSummary(HashMap<Tier, Vec<i64>>);

impl TryFrom<StateSummary<'_>> for TokenAllocationSummary {
    type Error = ContractError;

    fn try_from(state: StateSummary<'_>) -> Result<Self, Self::Error> {
        let this = bincode::deserialize_from(state.as_ref())
            .map_err(|err| ContractError::Deser(format!("{err}")))?;
        Ok(this)
    }
}

impl TryFrom<TokenAllocationSummary> for StateSummary<'static> {
    type Error = ContractError;

    fn try_from(summary: TokenAllocationSummary) -> Result<Self, Self::Error> {
        let serialized =
            bincode::serialize(&summary).map_err(|err| ContractError::Deser(format!("{err}")))?;
        Ok(StateSummary::from(serialized))
    }
}

pub type TokenAssignmentHash = [u8; 32];

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[must_use]
pub struct TokenAssignment {
    pub tier: Tier,
    pub time_slot: DateTime<Utc>,
    /// The assignment, the recipient decides whether this assignment is valid based on this field.
    /// This will often be a public key.
    pub assignee: Assignment,
    /// `(tier, issue_time, assignee)` must be signed by `generator_public_key`
    pub signature: Signature,
    /// A Blake2s256 hash of the message.
    pub assignment_hash: TokenAssignmentHash,
    /// Key to the contract holding the token records of the assignee.
    pub token_record: ContractInstanceId, // TODO: include this in the TokenAssignment itself
}

impl TokenAssignment {
    const TIER_SIZE: usize = std::mem::size_of::<Tier>();
    const TS_SIZE: usize = std::mem::size_of::<i64>();
    const ASSIGNEE_SIZE: usize = ed25519_dalek::PUBLIC_KEY_LENGTH;

    pub const SIGNED_MSG_SIZE: usize = Self::TIER_SIZE + Self::TS_SIZE + Self::ASSIGNEE_SIZE;

    /// The `(tier, issue_time, assignee)` tuple that has to be verified as bytes.
    pub fn to_be_signed(
        issue_time: &DateTime<Utc>,
        assigned_to: &Assignment,
        tier: Tier,
    ) -> [u8; Self::SIGNED_MSG_SIZE] {
        let mut cursor = Self::TIER_SIZE;
        let mut to_be_signed = [0; Self::SIGNED_MSG_SIZE];

        to_be_signed[..Self::TIER_SIZE].copy_from_slice(&(tier as u8).to_be_bytes());
        let timestamp = issue_time.timestamp();
        to_be_signed[cursor..cursor + Self::TS_SIZE].copy_from_slice(&timestamp.to_le_bytes());
        cursor += Self::TS_SIZE;
        to_be_signed[cursor..].copy_from_slice(assigned_to.as_bytes());

        to_be_signed
    }

    pub fn next_slot(&self) -> DateTime<Utc> {
        self.time_slot + Duration::from_std(self.tier.tier_duration()).unwrap()
    }

    pub fn previous_slot(&self) -> DateTime<Utc> {
        self.time_slot - Duration::from_std(self.tier.tier_duration()).unwrap()
    }
}

#[test]
fn to_be_signed_test() {
    let _to_be_signed = TokenAssignment::to_be_signed(
        &get_date(2021, 7, 28),
        &ed25519_dalek::PublicKey::from_bytes(&[1; ed25519_dalek::PUBLIC_KEY_LENGTH]).unwrap(),
        Tier::Day90,
    );
    // dbg!(_to_be_signed);
}

impl PartialOrd for TokenAssignment {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.time_slot.cmp(&other.time_slot))
    }
}

impl Ord for TokenAssignment {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.time_slot.cmp(&other.time_slot)
    }
}

impl TryFrom<StateDelta<'_>> for TokenAssignment {
    type Error = ContractError;

    fn try_from(state: StateDelta<'_>) -> Result<Self, Self::Error> {
        let this = bincode::deserialize_from(state.as_ref())
            .map_err(|err| ContractError::Deser(format!("{err}")))?;
        Ok(this)
    }
}

impl Display for TokenAssignment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let assignee = bs58::encode(&self.assignee).into_string();
        write!(
            f,
            "{{ {tier} @ {slot} for {assignee}}}",
            tier = self.tier,
            slot = self.time_slot,
        )
    }
}
