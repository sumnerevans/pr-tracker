# SPDX-License-Identifier: CC0-1.0
# SPDX-FileCopyrightText: 2021 Alyssa Ross <hi@alyssa.is>

CARGO = cargo
INSTALL = install
INSTALL_PROGRAM = $(INSTALL)
MKDIR_P = mkdir -p
PROFILE = release

prefix = /usr/local
exec_prefix = $(prefix)
bindir = $(exec_prefix)/bin

all: release
.PHONY: all

cargo-deps: vendor/github_schema.graphql src/merge_commit.graphql
.PHONY: cargo-deps

target/release/pr-tracker: cargo-deps
	$(CARGO) build --release

target/debug/pr-tracker: cargo-deps
	$(CARGO) build

check: cargo-deps
	$(CARGO) test
.PHONY: check

install-dirs:
	$(MKDIR_P) $(DESTDIR)$(bindir)
.PHONY: install-dirs

install: install-dirs target/$(PROFILE)/pr-tracker
	$(INSTALL_PROGRAM) target/$(PROFILE)/pr-tracker \
		$(DESTDIR)$(bindir)/pr-tracker
.PHONY: install

uninstall:
	rm -f $(DESTDIR)$(bindir)/pr-tracker
.PHONY: uninstall
