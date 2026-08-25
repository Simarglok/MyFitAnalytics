# Repository Agent Rules

These instructions apply to every coding agent working in this repository,
including Codex and Hermes.

## GitHub identity and credentials

- The canonical repository is `Simarglok/MyFitAnalytics` on `github.com`.
- Use the GitHub account `Simarglok` for direct pushes and pull requests in the
  canonical repository.
- GitHub CLI authentication is selected per host, not per repository. Treat
  `gh auth switch` as a global change for all local work targeting `github.com`.
- Before any remote mutation, run `gh auth status` and verify repository
  permissions with `gh api repos/Simarglok/MyFitAnalytics --jq '.permissions'`.
  A direct push requires `push: true` for the active account.
- If `Simarglok` is not available in `gh auth status`, stop and ask the user to
  complete `gh auth login --hostname github.com --web`. Never ask the user to
  paste a token into chat or store a token in the repository.
- Record which GitHub account was active before switching to `Simarglok`, and
  restore that account after the authorized remote operation is complete.
- `git config user.name` and `git config user.email` describe commit authorship;
  they do not prove GitHub authentication or repository permissions.
- Never commit credentials, tokens, `.env` files, or repository-specific shell
  configuration containing credentials.

## Git workflow and authority

- Before starting new work after a merge, fetch `origin` and create a new branch
  from the current `origin/main`. Do not continue new work on a merged branch.
- Keep changes scoped to the user's request and preserve unrelated user changes.
- Commits may be created locally as part of an authorized implementation task.
- Do not push, create a pull request, merge, tag, publish a release, delete a
  remote branch, or otherwise mutate GitHub state without explicit user
  authorization for that operation.
- Authorization to prepare or create a pull request includes pushing its source
  branch when necessary, but never includes merging the pull request.
- Do not create a fork as an authentication workaround unless the user explicitly
  chooses the fork-based workflow.
- Do not rewrite shared history or force-push unless the user explicitly requests
  it and the exact target has been verified.
- After a remote operation, report the account used, branch, commit, and resulting
  pull request or other remote object.
