// SPDX-License-Identifier: AGPL-3.0-or-later WITH GPL-3.0-linking-exception
// SPDX-FileCopyrightText: 2021 Alyssa Ross <hi@alyssa.is>

use std::borrow::Cow;
use std::collections::BTreeMap;

use once_cell::sync::Lazy;
use regex::{Regex, RegexSet};

const NEXT_BRANCH_TABLE: [(&str, &str); 8] = [
    (r"\Astaging\z", "staging-next"),
    (r"\Astaging-next\z", "master"),
    (r"\Amaster\z", "nixpkgs-unstable"),
    (r"\Amaster\z", "nixos-unstable-small"),
    (r"\Anixos-(.*)-small\z", "nixos-$1"),
    (r"\Arelease-([\d.]+)\z", "nixpkgs-$1-darwin"),
    (r"\Arelease-([\d.]+)\z", "nixos-$1-small"),
    (r"\Astaging-([\d.]*)\z", "release-$1"),
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

#[test]
fn test_next_branches() {
    let res = next_branches("release-20.09");
    assert_eq!(res, vec!["nixpkgs-20.09-darwin", "nixos-20.09-small"])
}
