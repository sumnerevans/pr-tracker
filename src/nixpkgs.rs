// SPDX-License-Identifier: AGPL-3.0-or-later WITH GPL-3.0-linking-exception
// SPDX-FileCopyrightText: 2021 Alyssa Ross <hi@alyssa.is>

use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::ffi::OsString;
use std::fmt::{self, Display, Formatter};
use std::os::unix::prelude::*;
use std::path::{Path, PathBuf};
use std::process::ExitStatus;

use async_std::io;
use async_std::process::{Command, Stdio};

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    ExitFailure(ExitStatus),
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        use Error::*;
        match self {
            Io(e) => write!(f, "git: {}", e),
            ExitFailure(e) => match e.code() {
                Some(code) => write!(f, "git exited {}", code),
                None => write!(f, "git killed by signal {}", e.signal().unwrap()),
            },
        }
    }
}

impl std::error::Error for Error {}

type Result<T, E = Error> = std::result::Result<T, E>;

fn check_status(status: ExitStatus) -> Result<()> {
    if status.success() {
        Ok(())
    } else {
        Err(Error::ExitFailure(status))
    }
}

pub struct Nixpkgs<'a> {
    path: &'a Path,
    remote_name: &'a Path,
}

impl<'a> Nixpkgs<'a> {
    pub fn new(path: &'a Path, remote_name: &'a Path) -> Self {
        Self { path, remote_name }
    }

    fn git_command(&self, subcommand: impl AsRef<OsStr>) -> Command {
        let mut command = Command::new("git");
        command.arg("-C");
        command.arg(&self.path);
        command.arg(subcommand);
        command
    }

    async fn git_branch_contains(&self, commit: &str) -> Result<Vec<u8>> {
        let output = self
            .git_command("branch")
            .args(&["-r", "--format=%(refname)", "--contains"])
            .arg(commit)
            .stderr(Stdio::inherit())
            .output()
            .await
            .map_err(Error::Io)?;

        check_status(output.status)?;

        Ok(output.stdout)
    }

    async fn git_fetch_nixpkgs(&self) -> Result<()> {
        // TODO: add refspecs
        self.git_command("fetch")
            .arg(&self.remote_name)
            .status()
            .await
            .map_err(Error::Io)
            .and_then(check_status)
    }

    pub async fn branches_containing_commit(
        &self,
        commit: &str,
        out: &mut BTreeSet<OsString>,
    ) -> Result<()> {
        let output = match self.git_branch_contains(commit).await {
            Err(Error::ExitFailure(status)) if status.code().is_some() => {
                eprintln!("pr-tracker: git branch --contains failed; updating branches");

                if let Err(e) = self.git_fetch_nixpkgs().await {
                    eprintln!("pr-tracker: fetching nixpkgs: {}", e);
                    // Carry on, because it might have fetched what we
                    // need before dying.
                }

                self.git_branch_contains(commit).await?
            }

            Ok(output) => output,
            Err(e) => return Err(e),
        };

        let mut prefix = PathBuf::from("refs/remotes/");
        prefix.push(&self.remote_name);

        for branch_name in output
            .split(|byte| *byte == b'\n')
            .filter(|b| !b.is_empty())
            .map(OsStr::from_bytes)
            .map(Path::new)
            .filter_map(|r| r.strip_prefix(&prefix).ok())
            .map(Into::into)
        {
            out.insert(branch_name);
        }

        Ok(())
    }
}
