# GitHub link with stars in the landing nav

## Summary

The repository is public now, so the landing nav links to it and shows its star count, read live from GitHub's API.

## Changes

- `landing/index.html`: GitHub link before Build it (mark, name, star icon, count), Phosphor Star icon, a fetch of `api.github.com/repos/yusupsupriyadi/opencompanion` kept 10 minutes in `sessionStorage`, and nav breakpoints (section links hide below 700px, the GitHub name below 880px, the brand wordmark below 400px).

## Decisions

- No count is written into the page: nothing shows until GitHub answers, and an error leaves the plain link.

## Verification

- `node --check`: ok. HTML balance: ok. The same fetch in Node returned `stargazers_count` 0. Nav widths checked by arithmetic per breakpoint.

## Limitations

- Not opened in a browser (needs the owner's approval). Unauthenticated API calls allow 60 per hour per visitor IP.

## Follow-up

- none
