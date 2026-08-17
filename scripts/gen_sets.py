#!/usr/bin/env python3
"""Regenerates data/sets/*.toml from their upstream sources.

Run from the repo root:

    python3 scripts/gen_sets.py

Every set is keyed on the LeetCode frontend id (`fid`), which is what `Problem` in the local
cache is filtered on. Sources give slugs (and sometimes a malformed id), so slugs are the join
key and fids come from LeetCode's own problem index. A slug that does not resolve fails the run
rather than being guessed at or quietly dropped.

Regeneration is all-or-nothing: every set is fetched, resolved, and rendered before anything is
written, so a failure part-way through cannot leave data/sets/ half-updated.

Only the standard library is used, so this needs no virtualenv.
"""

import csv
import datetime
import io
import json
import os
import sys
import urllib.request

USER_AGENT = "Mozilla/5.0 (compatible; leetctl-gen-sets/1.0)"

LEETCODE_PROBLEM_INDEX = "https://leetcode.com/api/problems/all/"
LEETCODE_GRAPHQL = "https://leetcode.com/graphql"
NEETCODE_SITE_DATA = (
    "https://raw.githubusercontent.com/neetcode-gh/leetcode/main/.problemSiteData.json"
)
COMPANY_CSV_BASE = (
    "https://raw.githubusercontent.com/hxu296/"
    "leetcode-company-wise-problems-2022/main/companies"
)

REPO_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUTPUT_DIR = os.path.join(REPO_ROOT, "data", "sets")

# `source_as_of` is the vintage of the DATA, not of the fetch — `generated_at` records the fetch.
# It is a plain date so nothing has to judge whether a set counts as "stale"; readers compare it
# against today themselves.
NEETCODE_SOURCE = {
    "source_url": "https://github.com/neetcode-gh/leetcode",
    "source_license": "MIT",
    "source_as_of": None,  # tracks upstream; filled in with the generation date
}
LEETCODE_SOURCE = {
    "source_url": "https://leetcode.com/studyplan/",
    "source_license": "Proprietary (LeetCode); problem identifiers only",
    "source_as_of": None,
}
# Not live LeetCode company tags — those are Premium-gated. This is a community-collected
# snapshot of which problems carried each company's tag in 2022.
COMPANY_SOURCE = {
    "source_url": "https://github.com/hxu296/leetcode-company-wise-problems-2022",
    "source_license": "MIT",
    "source_as_of": "2022",
}

# NeetCode's own data file carries `blind75` and `neetcode150` membership flags across its 450
# entries. There is no 250 flag and no public API exposing one, so NeetCode 250 is not shippable.
NEETCODE_SETS = [
    ("blind75", "Blind 75", "The original Blind 75 list, as tracked by NeetCode.", "blind75"),
    (
        "neetcode150",
        "NeetCode 150",
        "NeetCode's 150-problem core list, a superset of Blind 75.",
        "neetcode150",
    ),
    (
        "neetcode-all",
        "NeetCode All",
        "Every problem NeetCode tracks, grouped into 19 patterns.",
        None,
    ),
]

LEETCODE_STUDY_PLANS = [
    (
        "top-interview-150",
        "Top Interview 150",
        "LeetCode's official Top Interview 150 study plan.",
    ),
    (
        "top-100-liked",
        "Top 100 Liked",
        "LeetCode's official Top 100 Liked study plan.",
    ),
    (
        "leetcode-75",
        "LeetCode 75",
        "LeetCode's official 75-problem starter study plan.",
    ),
]

# LeetCode renamed these problems after the 2022 company data was collected. Each target was
# verified against the live problem index by frontend id, not guessed from the old name.
RENAMED_SLUGS = {
    "coin-change-2": "coin-change-ii",                                              # 518
    "increasing-subsequences": "non-decreasing-subsequences",                       # 491
    "add-and-search-word-data-structure-design":
        "design-add-and-search-words-data-structure",                               # 211
    "implement-strstr": "find-the-index-of-the-first-occurrence-in-a-string",        # 28
    "make-two-arrays-equal-by-reversing-sub-arrays":
        "make-two-arrays-equal-by-reversing-subarrays",                             # 1460
    "friend-circles": "number-of-provinces",                                        # 547
    "bulb-switcher-iii": "number-of-times-binary-string-is-prefix-aligned",          # 1375
}

COMPANIES = [
    ("google", "Google", "Google"),
    ("facebook", "Meta / Facebook", "Facebook"),
    ("amazon", "Amazon", "Amazon"),
    ("microsoft", "Microsoft", "Microsoft"),
]

# Generation fails if a set does not come out at exactly this size. For the curated lists the
# count is part of the name; for the company snapshots it is whatever the source held when the
# data was last reviewed. Either way a drift means a source changed and someone should look,
# rather than a set quietly shipping short.
EXPECTED_COUNTS = {
    "blind75": 75,
    "neetcode150": 150,
    "neetcode-all": 450,
    "top-interview-150": 150,
    "top-100-liked": 100,
    "leetcode-75": 75,
    "google": 488,
    "facebook": 371,
    "amazon": 592,
    "microsoft": 363,
}

# Slugs a source lists that deliberately have no LeetCode counterpart any more. Empty today:
# every slug across all ten sets resolves. An entry here must record why.
KNOWN_OMISSIONS = {}


def fetch(url, data=None):
    request = urllib.request.Request(url, data=data, headers={"User-Agent": USER_AGENT})
    if data is not None:
        request.add_header("Content-Type", "application/json")
    with urllib.request.urlopen(request, timeout=60) as response:
        return response.read()


def fetch_json(url, payload=None):
    body = json.dumps(payload).encode() if payload is not None else None
    return json.loads(fetch(url, body))


def slug_to_fid_index():
    """LeetCode's own problem index: title slug -> frontend id."""
    index = fetch_json(LEETCODE_PROBLEM_INDEX)
    return {
        pair["stat"]["question__title_slug"]: int(pair["stat"]["frontend_question_id"])
        for pair in index["stat_status_pairs"]
    }


def slug_from_problem_url(url):
    """https://leetcode.com/problems/two-sum/ -> two-sum"""
    return url.rstrip("/").rsplit("/", 1)[-1]


def neetcode_slugs(site_data, membership_flag):
    return [
        entry["link"].strip("/")
        for entry in site_data
        if membership_flag is None or entry.get(membership_flag)
    ]


def study_plan_slugs(plan_slug):
    query = (
        "query($slug: String!) { studyPlanV2Detail(planSlug: $slug) "
        "{ planSubGroups { questions { titleSlug } } } }"
    )
    payload = {"query": query, "variables": {"slug": plan_slug}}
    detail = fetch_json(LEETCODE_GRAPHQL, payload)["data"]["studyPlanV2Detail"]
    if detail is None:
        raise SystemExit(f"study plan {plan_slug!r} returned no detail")
    return [
        question["titleSlug"]
        for group in detail["planSubGroups"]
        for question in group["questions"]
    ]


def company_slugs(csv_name):
    text = fetch(f"{COMPANY_CSV_BASE}/{csv_name}.csv").decode()
    rows = csv.DictReader(io.StringIO(text))
    return [slug_from_problem_url(row["problem_link"]) for row in rows]


def toml_string(value):
    """TOML basic strings share JSON's escaping rules for the values we emit."""
    return json.dumps(value, ensure_ascii=False)


class GenerationError(Exception):
    """A set could not be generated correctly. Raised before anything is written."""


def resolve(slug, problem_slugs, fid_index):
    """Slug list -> [(fid, slug)]. Raises rather than silently dropping an entry."""
    resolved = []
    unresolved = []
    seen_fids = set()
    for problem_slug in problem_slugs:
        problem_slug = RENAMED_SLUGS.get(problem_slug, problem_slug)
        if problem_slug in KNOWN_OMISSIONS.get(slug, ()):
            continue
        fid = fid_index.get(problem_slug)
        if fid is None:
            unresolved.append(problem_slug)
            continue
        if fid in seen_fids:
            continue
        seen_fids.add(fid)
        resolved.append((fid, problem_slug))

    if unresolved:
        raise GenerationError(
            f"{slug}: {len(unresolved)} slug(s) do not resolve to a LeetCode frontend id: "
            f"{', '.join(unresolved)}.\n"
            f"  Either LeetCode renamed them — add the new slug to RENAMED_SLUGS, verified "
            f"against the live index by frontend id — or they were removed, in which case add "
            f"them to KNOWN_OMISSIONS[{slug!r}] with a reason."
        )

    expected = EXPECTED_COUNTS[slug]
    if len(resolved) != expected:
        raise GenerationError(
            f"{slug}: resolved {len(resolved)} problems, expected {expected}. "
            f"The source changed. Review the diff, then update EXPECTED_COUNTS."
        )

    return resolved


def render_set(slug, name, description, source, problem_slugs, fid_index, sort_by_fid):
    """Validate and render one set. Returns (slug, count, file contents) — writes nothing."""
    resolved = resolve(slug, problem_slugs, fid_index)
    if sort_by_fid:
        resolved.sort()

    generated_at = datetime.date.today().isoformat()
    lines = [
        "# @generated by scripts/gen_sets.py — do not edit by hand.",
        f"slug = {toml_string(slug)}",
        f"name = {toml_string(name)}",
        f"description = {toml_string(description)}",
        f"source_url = {toml_string(source['source_url'])}",
        f"source_license = {toml_string(source['source_license'])}",
        f"source_as_of = {toml_string(source['source_as_of'] or generated_at)}",
        f"generated_at = {toml_string(generated_at)}",
        "",
        "problems = [",
    ]
    lines += [f"  {{ fid = {fid}, slug = {toml_string(s)} }}," for fid, s in resolved]
    lines += ["]", ""]

    return slug, len(resolved), "\n".join(lines)


def write_all(rendered):
    """Replace every output file, each written atomically via a temporary file + rename."""
    for slug, count, contents in rendered:
        path = os.path.join(OUTPUT_DIR, f"{slug}.toml")
        temporary = f"{path}.tmp"
        with open(temporary, "w", encoding="utf-8") as handle:
            handle.write(contents)
        os.replace(temporary, path)
        print(f"{slug:<20} {count:>4} problems")


def main():
    os.makedirs(OUTPUT_DIR, exist_ok=True)

    print("fetching LeetCode problem index...")
    fid_index = slug_to_fid_index()
    print(f"  {len(fid_index)} problems indexed")

    print("fetching NeetCode site data...")
    site_data = fetch_json(NEETCODE_SITE_DATA)
    print(f"  {len(site_data)} problems")

    # Everything is fetched, validated and rendered first; only then is anything written, so a
    # failure in the tenth set cannot leave the first nine rewritten against a half-read source.
    rendered = []
    try:
        for slug, name, description, flag in NEETCODE_SETS:
            rendered.append(render_set(
                slug, name, description, NEETCODE_SOURCE,
                neetcode_slugs(site_data, flag), fid_index, sort_by_fid=False,
            ))

        for slug, name, description in LEETCODE_STUDY_PLANS:
            rendered.append(render_set(
                slug, name, description, LEETCODE_SOURCE,
                study_plan_slugs(slug), fid_index, sort_by_fid=False,
            ))

        for slug, name, csv_name in COMPANIES:
            description = (
                f"Problems carrying the {name} tag in a community-collected 2022 snapshot."
            )
            rendered.append(render_set(
                slug, name, description, COMPANY_SOURCE,
                company_slugs(csv_name), fid_index, sort_by_fid=True,
            ))
    except GenerationError as error:
        print(f"\nerror: {error}\nNothing was written.", file=sys.stderr)
        raise SystemExit(1)

    write_all(rendered)


if __name__ == "__main__":
    main()
