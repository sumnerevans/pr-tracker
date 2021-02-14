// SPDX-License-Identifier: AGPL-3.0-or-later WITH GPL-3.0-linking-exception
// SPDX-FileCopyrightText: 2021 Alyssa Ross <hi@alyssa.is>

mod branches;
mod github;
mod nixpkgs;
mod systemd;
mod tree;

use std::ffi::OsString;
use std::path::PathBuf;

use askama::Template;
use async_std::io;
use async_std::net::TcpListener;
use async_std::os::unix::io::FromRawFd;
use async_std::os::unix::net::UnixListener;
use async_std::pin::Pin;
use async_std::prelude::*;
use async_std::process::exit;
use futures_util::future::join_all;
use http_types::mime;
use once_cell::sync::Lazy;
use serde::Deserialize;
use structopt::StructOpt;
use tide::{Request, Response};

use github::{GitHub, PullRequestStatus};
use nixpkgs::Nixpkgs;
use systemd::{is_socket_inet, is_socket_unix, listen_fds};
use tree::Tree;

#[derive(StructOpt, Debug)]
struct Config {
    #[structopt(long, parse(from_os_str))]
    path: PathBuf,

    #[structopt(long, parse(from_os_str))]
    remote: PathBuf,

    #[structopt(long, parse(from_os_str))]
    user_agent: OsString,

    #[structopt(long)]
    source_url: String,

    #[structopt(long, default_value = "/")]
    mount: String,
}

static CONFIG: Lazy<Config> = Lazy::new(Config::from_args);

static GITHUB_TOKEN: Lazy<OsString> = Lazy::new(|| {
    use std::io::{stdin, BufRead, BufReader};
    use std::os::unix::prelude::*;

    let mut bytes = Vec::with_capacity(41);
    if let Err(e) = BufReader::new(stdin()).read_until(b'\n', &mut bytes) {
        eprintln!("pr-tracker: read: {}", e);
        exit(74)
    }
    if bytes.last() == Some(&b'\n') {
        bytes.pop();
    }
    OsString::from_vec(bytes)
});

#[derive(Debug, Default, Template)]
#[template(path = "page.html")]
struct PageTemplate {
    error: Option<String>,
    pr_number: Option<String>,
    closed: bool,
    tree: Option<Tree>,
    source_url: String,
}

#[derive(Debug, Deserialize)]
struct Query {
    pr: Option<String>,
}

async fn track_pr(pr_number: Option<String>, status: &mut u16, page: &mut PageTemplate) {
    let pr_number = match pr_number {
        Some(pr_number) => pr_number,
        None => return,
    };

    let pr_number_i64 = match pr_number.parse() {
        Ok(n) => n,
        Err(_) => {
            *status = 400;
            page.error = Some(format!("Invalid PR number: {}", pr_number));
            return;
        }
    };

    let github = GitHub::new(&GITHUB_TOKEN, &CONFIG.user_agent);

    let merge_info = match github.merge_info_for_nixpkgs_pr(pr_number_i64).await {
        Err(github::Error::NotFound) => {
            *status = 404;
            page.error = Some(format!("No such nixpkgs PR #{}.", pr_number_i64));
            return;
        }

        Err(e) => {
            *status = 500;
            page.error = Some(e.to_string());
            return;
        }

        Ok(info) => info,
    };

    page.pr_number = Some(pr_number);

    if matches!(merge_info.status, PullRequestStatus::Closed) {
        page.closed = true;
        return;
    }

    let nixpkgs = Nixpkgs::new(&CONFIG.path, &CONFIG.remote);
    let tree = Tree::make(merge_info.branch.to_string(), &merge_info.status, &nixpkgs).await;

    if let github::PullRequestStatus::Merged {
        merge_commit_oid, ..
    } = merge_info.status
    {
        if merge_commit_oid.is_none() {
            page.error = Some("For older PRs, GitHub doesn't tell us the merge commit, so we're unable to track this PR past being merged.".to_string());
        }
    }

    page.tree = Some(tree);
}

async fn handle_request<S>(request: Request<S>) -> http_types::Result<Response> {
    let mut status = 200;
    let mut page = PageTemplate {
        source_url: CONFIG.source_url.clone(),
        ..Default::default()
    };

    let pr_number = request.query::<Query>()?.pr;

    track_pr(pr_number, &mut status, &mut page).await;

    Ok(Response::builder(status)
        .content_type(mime::HTML)
        .body(page.render()?)
        .build())
}

#[async_std::main]
async fn main() {
    fn handle_error<T, E>(result: Result<T, E>, code: i32, message: impl AsRef<str>) -> T
    where
        E: std::error::Error,
    {
        match result {
            Ok(v) => return v,
            Err(e) => {
                eprintln!("pr-tracker: {}: {}", message.as_ref(), e);
                exit(code);
            }
        }
    }

    // Make sure arguments are parsed before starting server.
    let _ = *CONFIG;
    let _ = *GITHUB_TOKEN;

    let mut server = tide::new();
    let mut root = server.at(&CONFIG.mount);

    root.at("/").get(handle_request);

    let fd_count = handle_error(listen_fds(true), 71, "sd_listen_fds");

    if fd_count == 0 {
        eprintln!("pr-tracker: No listen file descriptors given");
        exit(64);
    }

    let mut listeners: Vec<Pin<Box<dyn Future<Output = _>>>> = Vec::new();

    for fd in (3..).into_iter().take(fd_count as usize) {
        let s = server.clone();
        if handle_error(is_socket_inet(fd), 74, "sd_is_socket_inet") {
            listeners.push(Box::pin(s.listen(unsafe { TcpListener::from_raw_fd(fd) })));
        } else if handle_error(is_socket_unix(fd), 74, "sd_is_socket_unix") {
            listeners.push(Box::pin(s.listen(unsafe { UnixListener::from_raw_fd(fd) })));
        } else {
            eprintln!("pr-tracker: file descriptor {} is not a socket", fd);
            exit(64);
        }
    }

    let errors: Vec<_> = join_all(listeners)
        .await
        .into_iter()
        .filter_map(io::Result::err)
        .collect();
    for error in errors.iter() {
        eprintln!("pr-tracker: listen: {}", error);
    }
    if !errors.is_empty() {
        exit(74);
    }
}
