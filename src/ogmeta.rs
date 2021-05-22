// SPDX-License-Identifier: AGPL-3.0-or-later WITH GPL-3.0-linking-exception
// SPDX-FileCopyrightText: 2021 Alyssa Ross <hi@alyssa.is>

use askama::Template;

use crate::Tree;

#[derive(Debug, Template)]
#[template(path = "ogmeta.html")]
pub struct Ogmeta {
    branch_name: String,
    accepted: Option<bool>,
    children: Vec<Ogmeta>,
}

impl Ogmeta {
    pub fn from_tree(tree: &Tree) -> Ogmeta {
        Ogmeta {
            branch_name: tree.branch_name.clone(),
            accepted: tree.accepted.clone(),
            children: tree.children.iter().map(|c| Ogmeta::from_tree(&c)).collect(),
        }
    }
}
