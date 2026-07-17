//! Ports of the TS engine's fast-check properties (test/engine.property.test.ts):
//! determinism and bloat-monotonicity.

use aispekt_core::types::AnalysisInput;
use aispekt_core::{analyze, report_to_json_pretty};
use proptest::prelude::*;

fn input_of(content: String) -> AnalysisInput {
    AnalysisInput {
        file_name: "CLAUDE.md".to_string(),
        content,
        repo: None,
    }
}

const BLOAT_LINES: [&str; 5] = [
    "Write clean code and follow best practices.",
    "Use 2 spaces for indentation in every file.",
    "IMPORTANT: YOU MUST always be careful.",
    "Handle errors appropriately as needed.",
    "Use meaningful names and keep it simple.",
];

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    /// analyze() is deterministic: identical input yields byte-identical report.
    #[test]
    fn determinism(content in "\\PC{0,400}") {
        let a = report_to_json_pretty(&analyze(&input_of(content.clone())));
        let b = report_to_json_pretty(&analyze(&input_of(content)));
        prop_assert_eq!(a, b);
    }

    /// Appending bloat lines never raises the score (penalty-only scoring).
    #[test]
    fn bloat_monotonic(content in "\\PC{0,300}", picks in proptest::collection::vec(0usize..5, 1..6)) {
        let base = analyze(&input_of(content.clone())).score;
        let mut bloated = content;
        for p in picks {
            bloated.push('\n');
            bloated.push_str(BLOAT_LINES[p]);
        }
        let after = analyze(&input_of(bloated)).score;
        prop_assert!(after <= base, "bloat raised score: {base} -> {after}");
    }
}
