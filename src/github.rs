// SPDX-License-Identifier: AGPL-3.0-or-later WITH GPL-3.0-linking-exception
// SPDX-FileCopyrightText: 2021 Alyssa Ross <hi@alyssa.is>

use std::ffi::OsStr;
use std::fmt::{self, Display, Formatter};
use std::os::unix::ffi::OsStrExt;

use graphql_client::GraphQLQuery;
use serde::Deserialize;
use surf::http::headers::HeaderValue;
use surf::StatusCode;

type GitObjectID = String;

#[derive(Debug)]
pub enum Error {
    NotFound,
    Serialization(serde_json::Error),
    Request(surf::Error),
    Response(StatusCode),
    Deserialization(http_types::Error),
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        use Error::*;
        match self {
            NotFound => write!(f, "Not found"),
            Serialization(e) => write!(f, "Serialization error: {}", e),
            Request(e) => write!(f, "Request error: {}", e),
            Response(s) => write!(f, "Unexpected response status: {}", s),
            Deserialization(e) => write!(f, "Deserialization error: {}", e),
        }
    }
}

impl std::error::Error for Error {}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "vendor/github_schema.graphql",
    query_path = "src/merge_commit.graphql",
    response_derives = "Debug"
)]
struct MergeCommitQuery;

#[derive(Debug, Deserialize)]
struct GitHubGraphQLResponse<D> {
    data: D,
}

#[derive(Debug)]
pub enum PullRequestStatus {
    Open,
    Closed,
    Merged {
        /// This field is optional because GitHub doesn't provide us with this information
        /// for PRs merged before around March 2016.
        merge_commit_oid: Option<String>,
    },
}

#[derive(Debug)]
pub struct MergeInfo {
    pub branch: String,
    pub status: PullRequestStatus,
}

pub struct GitHub<'a> {
    token: &'a OsStr,
    user_agent: &'a OsStr,
}

impl<'a> GitHub<'a> {
    pub fn new(token: &'a OsStr, user_agent: &'a OsStr) -> Self {
        Self { token, user_agent }
    }

    fn authorization_header(&self) -> Result<HeaderValue, surf::Error> {
        let mut value = b"bearer ".to_vec();
        value.extend_from_slice(self.token.as_bytes());
        Ok(HeaderValue::from_bytes(value)?)
    }

    pub async fn merge_info_for_nixpkgs_pr(&self, pr: i64) -> Result<MergeInfo, Error> {
        let query = MergeCommitQuery::build_query(merge_commit_query::Variables {
            owner: "NixOS".to_string(),
            repo: "nixpkgs".to_string(),
            number: pr,
        });

        let response = surf::post("https://api.github.com/graphql")
            .header("Accept", "application/vnd.github.merge-info-preview+json")
            .header(
                "User-Agent",
                HeaderValue::from_bytes(self.user_agent.as_bytes().to_vec())
                    .map_err(Error::Request)?,
            )
            .header(
                "Authorization",
                self.authorization_header().map_err(Error::Request)?,
            )
            .body(serde_json::to_vec(&query).map_err(Error::Serialization)?)
            .send()
            .await
            .map_err(Error::Request)?;

        let status = response.status();
        if status == StatusCode::NotFound || status == StatusCode::Gone {
            return Err(Error::NotFound);
        } else if !status.is_success() {
            return Err(Error::Response(status));
        }

        let data: GitHubGraphQLResponse<merge_commit_query::ResponseData> = dbg!(response)
            .body_json()
            .await
            .map_err(Error::Deserialization)?;

        let pr = data
            .data
            .repository
            .and_then(|repo| repo.pull_request)
            .ok_or(Error::NotFound)?;

        Ok(MergeInfo {
            branch: pr.base_ref_name,
            status: if pr.merged {
                PullRequestStatus::Merged {
                    merge_commit_oid: pr.merge_commit.map(|commit| commit.oid),
                }
            } else if pr.closed {
                PullRequestStatus::Closed
            } else {
                PullRequestStatus::Open
            },
        })
    }
}
