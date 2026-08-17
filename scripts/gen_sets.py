#!/usr/bin/env python3
"""Regenerates data/sets/*.toml from their upstream sources.

Run from the repo root:

    python3 scripts/gen_sets.py

Every set is keyed on the LeetCode frontend id (`fid`), which is what `Problem` in the local
cache is filtered on. Sources give slugs (and sometimes a malformed id), so slugs are the join
key and fids come from LeetCode's own problem index. A slug that does not resolve is reported
and skipped rather than guessed at.

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

NEETCODE_SOURCE = {
    "source_url": "https://github.com/neetcode-gh/leetcode",
    "source_license": "MIT",
    "stale": False,
}
LEETCODE_SOURCE = {
    "source_url": "https://leetcode.com/studyplan/",
    "source_license": "LeetCode study plan, fetched from leetcode.com",
    "stale": False,
}
# The company lists are community-scraped 2022 frequency data, not live LeetCode company tags
# (those are Premium-gated). They are marked stale so `leetctl sets` can say so out loud.
COMPANY_SOURCE = {
    "source_url": "https://github.com/hxu296/leetcode-company-wise-problems-2022",
    "source_license": "MIT",
    "stale": True,
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


def write_set(slug, name, description, source, problem_slugs, fid_index, sort_by_fid):
    resolved = []
    unresolved = []
    seen = set()
    for problem_slug in problem_slugs:
        problem_slug = RENAMED_SLUGS.get(problem_slug, problem_slug)
        fid = fid_index.get(problem_slug)
        if fid is None:
            unresolved.append(problem_slug)
            continue
        if fid in seen:
            continue
        seen.add(fid)
        resolved.append((fid, problem_slug))

    if sort_by_fid:
        resolved.sort()

    lines = [
        f"# @generated by scripts/gen_sets.py — do not edit by hand.",
        f"slug = {toml_string(slug)}",
        f"name = {toml_string(name)}",
        f"description = {toml_string(description)}",
        f"source_url = {toml_string(source['source_url'])}",
        f"source_license = {toml_string(source['source_license'])}",
        f"generated_at = {toml_string(datetime.date.today().isoformat())}",
        f"stale = {'true' if source['stale'] else 'false'}",
        "",
        "problems = [",
    ]
    lines += [f"  {{ fid = {fid}, slug = {toml_string(s)} }}," for fid, s in resolved]
    lines += ["]", ""]

    path = os.path.join(OUTPUT_DIR, f"{slug}.toml")
    with open(path, "w", encoding="utf-8") as handle:
        handle.write("\n".join(lines))

    status = f"{slug:<20} {len(resolved):>4} problems"
    if unresolved:
        status += f"  ({len(unresolved)} unresolved: {', '.join(unresolved)})"
    print(status)
    return unresolved


def main():
    os.makedirs(OUTPUT_DIR, exist_ok=True)

    print("fetching LeetCode problem index...")
    fid_index = slug_to_fid_index()
    print(f"  {len(fid_index)} problems indexed")

    print("fetching NeetCode site data...")
    site_data = fetch_json(NEETCODE_SITE_DATA)
    print(f"  {len(site_data)} problems")

    all_unresolved = {}

    for slug, name, description, flag in NEETCODE_SETS:
        all_unresolved[slug] = write_set(
            slug, name, description, NEETCODE_SOURCE,
            neetcode_slugs(site_data, flag), fid_index, sort_by_fid=False,
        )

    for slug, name, description in LEETCODE_STUDY_PLANS:
        all_unresolved[slug] = write_set(
            slug, name, description, LEETCODE_SOURCE,
            study_plan_slugs(slug), fid_index, sort_by_fid=False,
        )

    for slug, name, csv_name in COMPANIES:
        description = (
            f"Problems tagged {name} in community-collected 2022 interview frequency data."
        )
        all_unresolved[slug] = write_set(
            slug, name, description, COMPANY_SOURCE,
            company_slugs(csv_name), fid_index, sort_by_fid=True,
        )

    total_unresolved = sum(len(v) for v in all_unresolved.values())
    if total_unresolved:
        print(f"\n{total_unresolved} slug(s) could not be resolved to a frontend id.", file=sys.stderr)
        print("They were skipped. Usually this means the problem was removed from LeetCode.", file=sys.stderr)


if __name__ == "__main__":
    main()
