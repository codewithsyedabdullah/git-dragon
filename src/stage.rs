//! Evolution stages and the contribution thresholds that select them.

use std::fmt;

/// The six evolution stages of the dragon, from egg to legend.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Stage {
    Egg,
    Baby,
    Young,
    Adolescent,
    Adult,
    Legendary,
}

impl Stage {
    /// Every stage in evolution order.
    pub const ALL: [Stage; 6] = [
        Stage::Egg,
        Stage::Baby,
        Stage::Young,
        Stage::Adolescent,
        Stage::Adult,
        Stage::Legendary,
    ];

    /// Map a contribution total to its stage.
    ///
    /// | Contributions | Stage      |
    /// |---------------|------------|
    /// | 0             | Egg        |
    /// | 1–149         | Baby       |
    /// | 150–499       | Young      |
    /// | 500–1,199     | Adolescent |
    /// | 1,200–2,499   | Adult      |
    /// | 2,500+        | Legendary  |
    pub fn from_contributions(contributions: u64) -> Self {
        match contributions {
            0 => Stage::Egg,
            1..=149 => Stage::Baby,
            150..=499 => Stage::Young,
            500..=1_199 => Stage::Adolescent,
            1_200..=2_499 => Stage::Adult,
            _ => Stage::Legendary,
        }
    }

    /// Zero-based position in the evolution order.
    pub fn index(self) -> usize {
        self as usize
    }

    /// Human-readable stage name.
    pub fn name(self) -> &'static str {
        match self {
            Stage::Egg => "Dragon Egg",
            Stage::Baby => "Baby Dragon",
            Stage::Young => "Young Dragon",
            Stage::Adolescent => "Adolescent Dragon",
            Stage::Adult => "Adult Dragon",
            Stage::Legendary => "Legendary Dragon",
        }
    }

    /// Short file-friendly identifier (`egg`, `baby`, ...).
    pub fn slug(self) -> &'static str {
        match self {
            Stage::Egg => "egg",
            Stage::Baby => "baby",
            Stage::Young => "young",
            Stage::Adolescent => "adolescent",
            Stage::Adult => "adult",
            Stage::Legendary => "legendary",
        }
    }

    /// Inclusive contribution range that maps to this stage (`None` upper bound = open-ended).
    pub fn range(self) -> (u64, Option<u64>) {
        match self {
            Stage::Egg => (0, Some(0)),
            Stage::Baby => (1, Some(149)),
            Stage::Young => (150, Some(499)),
            Stage::Adolescent => (500, Some(1_199)),
            Stage::Adult => (1_200, Some(2_499)),
            Stage::Legendary => (2_500, None),
        }
    }

    /// Whether this stage has a body that can breathe fire.
    pub fn breathes_fire(self) -> bool {
        matches!(self, Stage::Adolescent | Stage::Adult | Stage::Legendary)
    }
}

impl fmt::Display for Stage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Stage {}: {}", self.index(), self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thresholds_are_exact() {
        let cases = [
            (0, Stage::Egg),
            (1, Stage::Baby),
            (149, Stage::Baby),
            (150, Stage::Young),
            (499, Stage::Young),
            (500, Stage::Adolescent),
            (1_199, Stage::Adolescent),
            (1_200, Stage::Adult),
            (2_499, Stage::Adult),
            (2_500, Stage::Legendary),
            (u64::MAX, Stage::Legendary),
        ];
        for (n, stage) in cases {
            assert_eq!(Stage::from_contributions(n), stage, "contributions={n}");
        }
    }

    #[test]
    fn ranges_agree_with_thresholds() {
        for stage in Stage::ALL {
            let (lo, hi) = stage.range();
            assert_eq!(Stage::from_contributions(lo), stage);
            if let Some(hi) = hi {
                assert_eq!(Stage::from_contributions(hi), stage);
            }
        }
    }
}
