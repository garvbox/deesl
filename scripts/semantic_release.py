#!/usr/bin/env python3
"""Semantic release helpers for deesl.

Two subcommands:

- ``check``: validate that every commit in a pull request follows the
  conventional commit format, then label the PR with the resulting release
  type (``release: major``, ``release: minor``, ``release: patch`` or
  ``release: none``). Exits non-zero if any commit is non-conventional.

- ``bump``: determine the release type from the commits since the last tag,
  bump the version in ``Cargo.toml``, and print the new version to stdout.
  Prints nothing and exits zero when no release is needed. ``Cargo.lock`` is
  left for ``cargo check`` to sync.
"""

import argparse
import json
import os
import re
import subprocess
import sys

CONVENTIONAL_TYPES = {
    "feat",
    "fix",
    "chore",
    "docs",
    "refactor",
    "perf",
    "test",
    "build",
    "ci",
    "style",
    "revert",
}

COMMIT_RE = re.compile(
    r"^(?P<type>[a-z]+)(?:\((?P<scope>[^)\n]+)\))?(?P<breaking>!)?: (?P<description>.+)$"
)

BREAKING_RE = re.compile(r"BREAKING[ -]CHANGE:")

PATCH_TYPES = {"fix", "ci"}

LABEL_COLORS = {
    "major": "b60205",
    "minor": "0366d6",
    "patch": "0e8a16",
    "none": "cccccc",
}


def parse_commit(message):
    first_line = message.split("\n", 1)[0]
    match = COMMIT_RE.match(first_line)
    if not match:
        return None
    commit_type = match.group("type")
    if commit_type not in CONVENTIONAL_TYPES:
        return None
    breaking = bool(match.group("breaking")) or bool(BREAKING_RE.search(message))
    return {"type": commit_type, "breaking": breaking}


def release_type(messages):
    has_breaking = False
    has_feat = False
    has_patch = False
    for message in messages:
        parsed = parse_commit(message)
        if parsed is None:
            continue
        if parsed["breaking"]:
            has_breaking = True
        elif parsed["type"] == "feat":
            has_feat = True
        elif parsed["type"] in PATCH_TYPES:
            has_patch = True
    if has_breaking:
        return "major"
    if has_feat:
        return "minor"
    if has_patch:
        return "patch"
    return None


def bump_version(current, release_type):
    major, minor, patch = (int(part) for part in current.split("."))
    if release_type == "major":
        return f"{major + 1}.0.0"
    if release_type == "minor":
        return f"{major}.{minor + 1}.0"
    if release_type == "patch":
        return f"{major}.{minor}.{patch + 1}"
    return current


def read_version():
    with open("Cargo.toml") as f:
        for line in f:
            match = re.match(r'^version\s*=\s*"([^"]+)"', line)
            if match:
                return match.group(1)
    raise RuntimeError("version not found in Cargo.toml")


def write_version(new_version):
    with open("Cargo.toml") as f:
        content = f.read()
    content = re.sub(
        r'^version\s*=\s*"[^"]+"',
        f'version = "{new_version}"',
        content,
        count=1,
        flags=re.MULTILINE,
    )
    with open("Cargo.toml", "w") as f:
        f.write(content)


def commits_since_last_tag():
    try:
        last_tag = subprocess.check_output(
            ["git", "describe", "--tags", "--abbrev=0"], text=True
        ).strip()
        range_spec = f"{last_tag}..HEAD"
    except subprocess.CalledProcessError:
        range_spec = "HEAD"

    output = subprocess.check_output(
        ["git", "log", "--format=%B%x00", range_spec]
    ).decode("utf-8", errors="replace")
    return [message.strip() for message in output.split("\x00") if message.strip()]


def get_pr_number():
    event_path = os.environ.get("GITHUB_EVENT_PATH")
    if event_path:
        with open(event_path) as f:
            event = json.load(f)
        pr = event.get("pull_request", {})
        if "number" in pr:
            return pr["number"]

    ref = os.environ.get("GITHUB_REF", "")
    match = re.match(r"refs/pull/(\d+)/", ref)
    if match:
        return int(match.group(1))

    raise RuntimeError("could not determine pull request number")


def apply_label(repo, pr, label_name):
    from github.GithubException import UnknownObjectException

    try:
        label = repo.get_label(label_name)
    except UnknownObjectException:
        label = repo.create_label(label_name, LABEL_COLORS[label_name.split(": ")[1]])

    for existing in pr.get_labels():
        if existing.name.startswith("release:") and existing.name != label_name:
            pr.remove_from_labels(existing)

    pr.add_to_labels(label)


def check():
    from github import Github

    token = os.environ["GITHUB_TOKEN"]
    repo_name = os.environ["GITHUB_REPOSITORY"]
    pr_number = get_pr_number()

    gh = Github(token)
    repo = gh.get_repo(repo_name)
    pr = repo.get_pull(pr_number)

    messages = [commit.commit.message for commit in pr.get_commits()]

    non_conventional = [m for m in messages if parse_commit(m) is None]
    if non_conventional:
        print("The following commits do not follow the conventional commit format:")
        for message in non_conventional:
            print(f"  - {message.splitlines()[0]}")
        print()
        print("Expected format: type(scope)!: description")
        print(f"Allowed types: {', '.join(sorted(CONVENTIONAL_TYPES))}")
        sys.exit(1)

    rtype = release_type(messages) or "none"
    apply_label(repo, pr, f"release: {rtype}")
    print(f"Release type: {rtype}")


def bump():
    current = read_version()
    rtype = release_type(commits_since_last_tag())
    if rtype is None:
        print("No release required", file=sys.stderr)
        return

    new_version = bump_version(current, rtype)
    write_version(new_version)
    print(f"Release type: {rtype}, new version: {new_version}", file=sys.stderr)
    print(new_version)


def main():
    parser = argparse.ArgumentParser()
    subparsers = parser.add_subparsers(dest="command", required=True)
    subparsers.add_parser("check")
    subparsers.add_parser("bump")
    args = parser.parse_args()

    if args.command == "check":
        check()
    elif args.command == "bump":
        bump()


if __name__ == "__main__":
    main()
