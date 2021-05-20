// SPDX-License-Identifier: AGPL-3.0-or-later WITH GPL-3.0-linking-exception
// SPDX-FileCopyrightText: 2021 Alyssa Ross <hi@alyssa.is>
// SPDX-FileCopyrightText: 2021 Sumner Evans <me@sumnerevans.com>

use std::ffi::OsStr;
use std::fmt::{self, Display, Formatter};
use std::os::unix::ffi::OsStrExt;

use graphql_client::GraphQLQuery;
use serde::Deserialize;
use surf::http::headers::HeaderValue;
use surf::StatusCode;

// ISO 8601 dates can be compared chronologically simply by comparing
// them lexicographically, so representing them as strings and
// comparing them as strings works just fine.  (As long as GitHub
// never starts returning dates in non-UTC timezones!)
type DateTime = String;

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

// Prior to some time in October 2013, GitHub changes from showing the
// GraphQL API us a fake merge commit that isn't actually reachable in
// the branch, to showing a null merge commit.
//
// The earliest merge I could find with the null behaviour was
// nixpkgs#1050, which was merged at the time below.
// The most recent merge I could find before that, where GitHub
// returns a fake merge commit, is nixpkgs#1049, which was merged at
// 2013-10-06T14:05:21Z.  So the behaviour change happens somewhere in
// the two weeks between those dates.  By looking at other GitHub
// repositories (or even just more closely at Nixpkgs), we could
// refine this value, but since we treat fake merge commits as null
// merge commits, there's not much point.
//
// The change from null merge commits to real merge commit data
// happens in March 2016 (we don't need to check for that by date).
const FIRST_KNOWN_NULL_MERGE_COMMIT: &str = "2013-10-20T15:50:06Z";

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "vendor/github_schema.graphql",
    query_path = "src/pr_info.graphql",
    response_derives = "Debug"
)]
struct PrInfoQuery;

type PullRequest = pr_info_query::PrInfoQueryRepositoryPullRequest;

impl PullRequest {
    fn merge_commit_oid(&self) -> Option<&str> {
        if self.merged_at.as_ref()?.as_str() < FIRST_KNOWN_NULL_MERGE_COMMIT {
            return None;
        }

        Some(&self.merge_commit.as_ref()?.oid)
    }
}

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
pub struct PrInfo {
    pub branch: String,
    pub title: String,
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

    pub async fn pr_info_for_nixpkgs_pr(&self, pr: i64) -> Result<PrInfo, Error> {
        let query = PrInfoQuery::build_query(pr_info_query::Variables {
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

        let data: GitHubGraphQLResponse<pr_info_query::ResponseData> = dbg!(response)
            .body_json()
            .await
            .map_err(Error::Deserialization)?;

        let pr = data
            .data
            .repository
            .and_then(|repo| repo.pull_request)
            .ok_or(Error::NotFound)?;

        let status = if pr.merged {
            let merge_commit_oid = pr.merge_commit_oid().map(Into::into);
            PullRequestStatus::Merged { merge_commit_oid }
        } else if pr.closed {
            PullRequestStatus::Closed
        } else {
            PullRequestStatus::Open
        };

        Ok(PrInfo {
            branch: pr.base_ref_name,
            title: pr.title,
            status,
        })
    }
}
