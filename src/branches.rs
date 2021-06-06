// SPDX-License-Identifier: AGPL-3.0-or-later WITH GPL-3.0-linking-exception
// SPDX-FileCopyrightText: 2021 Alyssa Ross <hi@alyssa.is>

use std::borrow::Cow;
use std::collections::BTreeMap;

use once_cell::sync::Lazy;
use regex::{Regex, RegexSet};

const NEXT_BRANCH_TABLE: [(&str, &str); 10] = [
    (r"\Astaging\z", "staging-next"),
    (r"\Astaging-next\z", "master"),
    (r"\Astaging-next-([\d.]+)\z", "release-$1"),
    (r"\Amaster\z", "nixpkgs-unstable"),
    (r"\Amaster\z", "nixos-unstable-small"),
    (r"\Anixos-(.*)-small\z", "nixos-$1"),
    (r"\Arelease-([\d.]+)\z", "nixpkgs-$1-darwin"),
    (r"\Arelease-([\d.]+)\z", "nixos-$1-small"),
    (r"\Astaging-((1.|20)\.\d{2})\z", "release-$1"),
    (r"\Astaging-((2[1-9]|[3-90].)\.\d{2})\z", "staging-next-$1"),
];

static BRANCH_NEXTS: Lazy<BTreeMap<&str, Vec<&str>>> = Lazy::new(|| {
    NEXT_BRANCH_TABLE
        .iter()
        .fold(BTreeMap::new(), |mut map, (pattern, next)| {
            map.entry(pattern).or_insert_with(Vec::new).push(next);
            map
        })
});

static BRANCH_NEXTS_BY_INDEX: Lazy<Vec<&Vec<&str>>> = Lazy::new(|| BRANCH_NEXTS.values().collect());

static BRANCH_PATTERNS: Lazy<Vec<Regex>> = Lazy::new(|| {
    BRANCH_NEXTS
        .keys()
        .copied()
        .map(Regex::new)
        .map(Result::unwrap)
        .collect()
});

static BRANCH_REGEXES: Lazy<RegexSet> = Lazy::new(|| RegexSet::new(BRANCH_NEXTS.keys()).unwrap());

pub fn next_branches(branch: &str) -> Vec<Cow<str>> {
    BRANCH_REGEXES
        .matches(branch)
        .iter()
        .flat_map(|index| {
            let regex = BRANCH_PATTERNS.get(index).unwrap();
            BRANCH_NEXTS_BY_INDEX
                .get(index)
                .unwrap()
                .iter()
                .map(move |next| regex.replace(branch, *next))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn staging_18_03() {
	let res = next_branches("staging-18.03");
	assert_eq!(res, vec!["release-18.03"]);
    }

    #[test]
    fn staging_20_09() {
	let res = next_branches("staging-20.09");
	assert_eq!(res, vec!["release-20.09"]);
    }

    #[test]
    fn staging_21_05() {
	let res = next_branches("staging-21.05");
	assert_eq!(res, vec!["staging-next-21.05"]);
    }

    #[test]
    fn staging_30_05() {
	let res = next_branches("staging-30.05");
	assert_eq!(res, vec!["staging-next-30.05"]);
    }

    #[test]
    fn staging_00_11() {
	let res = next_branches("staging-00.11");
	assert_eq!(res, vec!["staging-next-00.11"]);
    }

    #[test]
    fn staging_next_21_05() {
	let res = next_branches("staging-next-21.05");
	assert_eq!(res, vec!["release-21.05"]);
    }

    #[test]
    fn release_20_09() {
	let res = next_branches("release-20.09");
	assert_eq!(res, vec!["nixpkgs-20.09-darwin", "nixos-20.09-small"]);
    }
}
