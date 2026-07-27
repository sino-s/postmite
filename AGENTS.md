# Postmite Agent Workflow

This repository is implemented from GitHub Issues. An agent must not begin implementation from a chat request, an epic, or an untracked idea. Every change starts from one implementation issue and results in one commit and one pull request.

## Sources of truth

1. The assigned GitHub Issue defines the deliverable, dependencies, scope, acceptance criteria, and verification.
2. `CONTEXT.md` defines canonical domain language.
3. `AGENTS.md` defines the execution workflow.
4. Local files under `docs/` may provide additional design context, but `docs/*` is intentionally ignored and may not exist in every clone. An Issue must therefore be self-contained.

If these sources conflict, stop and report the conflict on the Issue instead of guessing.

## Selecting work

Only take an Issue when all of the following are true:

- It has the `agent-ready` label.
- It does not have `blocked` or `in-progress`.
- Every Issue listed under `Depends on` is closed.
- Its parent Epic is part of the active Milestone.
- Its acceptance criteria can be completed in one pull request.

Claim work atomically:

1. Assign yourself or the designated bot identity.
2. Remove `agent-ready`.
3. Add `in-progress`.
4. Comment that work has started and name the branch.
5. Create `issue-<number>-<slug>` from the latest `main`.

If claiming fails or another actor claimed it first, do not work on that Issue.

## Mandatory plan

Before editing implementation files, create:

```text
plans/<issue-number>-<slug>.md
```

The plan is transient and must never be staged. It is deliberately not ignored so `git status` exposes accidental leftovers.

The plan file is also the complete commit message. It must use this structure:

```md
<type>(<scope>): <short description>

Issue: #<number>

## Outcome

<observable result>

## Scope

- <included work>

## Steps

1. <implementation step>

## Verification

- <exact command or manual check>

Refs: #<number>
```

The first line must comply with [Conventional Commits 1.0.0](https://www.conventionalcommits.org/ja/v1.0.0/). Allowed types are:

- `feat`: user-visible capability
- `fix`: bug fix
- `refactor`: behavior-preserving restructuring
- `perf`: performance improvement
- `test`: test-only change
- `docs`: tracked documentation
- `build`: build system or dependency change
- `ci`: continuous integration change
- `chore`: repository maintenance
- `revert`: revert of an earlier commit

Use `!` and a `BREAKING CHANGE:` footer when applicable. Keep the scope short and stable, such as `http`, `workspace`, `import`, `ui`, or `repo`.

## Implementation

- Implement only the Issue scope and the approved plan.
- Do not include opportunistic cleanup or unrelated user changes.
- Keep Rust as the owner of persisted data, HTTP execution, secrets, and filesystem access.
- Keep the WebView behind typed Tauri IPC; never replace Rust request execution with browser `fetch`.
- Preserve workspace ownership boundaries and ordered duplicate Query/Header fields.
- Never place Secret values in SQLite, logs, diagnostics, exports, snapshots, fixtures, IPC errors, or screenshots.
- Add or update tests in the same change.
- Check performance and accessibility whenever the Issue labels require them.
- Update the plan before implementation if the approach changes materially, and explain the change on the Issue.

## Verification and commit

Run every check listed in the Issue and plan. Before committing:

1. Re-read the Issue and confirm every acceptance criterion.
2. Inspect `git diff` and remove unrelated changes.
3. Confirm the plan is not staged.
4. Confirm no Secret or generated artifact is staged.
5. Stage only the implementation files.
6. Run `git diff --check --cached`.
7. Commit with the plan file itself:

```bash
git commit -F plans/<issue-number>-<slug>.md
```

This makes the plan content the commit message verbatim, including its Conventional Commit header. Do not rewrite, summarize, or append a different commit message.

After the commit succeeds, delete the plan file. Confirm that:

- `plans/<issue-number>-<slug>.md` no longer exists.
- The plan was not included in the commit.
- `git status --short` shows no task-related leftovers.
- `git log -1 --format=%B` matches the deleted plan content exactly.

If review requires code changes, amend the same commit and continue using the same plan message. Do not add fixup commits.

## Pull request and handoff

Push the issue branch and open one pull request:

- Use the same Conventional Commit header as the pull request title.
- Include `Closes #<number>`.
- List verification results and any residual risk.
- Do not claim that a check passed unless it was run.
- Add `needs-review` to the Issue and remove `in-progress`.

The reviewing agent must not be the implementing agent. It checks the diff against the Issue, plan-shaped commit message, tests, security boundaries, and performance budget.

After merge:

1. Let the pull request close the implementation Issue.
2. Delete the branch.
3. Check dependent Issues.
4. Add `agent-ready` only when all their dependencies are closed.
5. Close the parent Epic only after all child Issues are closed and its slice acceptance criteria pass.

## Prohibited shortcuts

- Do not implement directly from an Epic.
- Do not take an Issue without `agent-ready`.
- Do not work around an unresolved dependency.
- Do not commit a plan file.
- Do not use a commit message other than the exact plan content.
- Do not combine multiple implementation Issues into one commit.
- Do not close an Issue without the required verification evidence.
- Do not commit ignored local design documents under `docs/`.
