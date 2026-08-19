# Cookies

leetctl talks to LeetCode as you, so it needs your session cookies. There are three ways to provide them.

## 1. Automatic (Chrome)

If `csrf` and `session` are both empty in `leetcode.toml`, leetctl reads the cookies straight from **Chrome's** cookie store for the configured `site`. Just sign in to LeetCode in Chrome and run any command — there is nothing else to set up.

This is the only automatic path, and it is Chrome-only. It works on **macOS and Linux**; on **Windows** Chrome's cookie encryption isn't read automatically yet, so use the manual setup below. The same applies if you use a different browser, or Chrome's cookie store can't be read.

> If you see a "not logged in to Chrome" error, either sign in to LeetCode in Chrome or set the cookies manually.

Chrome's Cloudflare cookies (`cf_clearance`, `__cf_bm`, `_cfuvid`) are read along with the two identity cookies, and leetctl presents the same Chrome's user agent. See [Cloudflare challenges](#cloudflare-challenges) for why.

## 2. Manual (any browser)

Copy the two cookie values into `leetcode.toml`:

```toml
[cookies]
csrf = '<your-leetcode-csrf-token>'
session = '<your-leetcode-session-key>'
site = 'leetcode.com'
```

To find them — for example in Firefox, after logging in to LeetCode:

1. Press <kbd>F12</kbd> and open the **Storage** tab.
2. Expand **Cookies** and select `https://leetcode.com`.
3. Copy the `Value` of `csrftoken` into `csrf` and `LEETCODE_SESSION` into `session`.

The same values are available under DevTools in any Chromium browser (Application → Cookies).

## 3. Environment variables

To keep secrets out of `leetcode.toml`, leave `csrf` and `session` empty and export them instead. Environment variables override whatever is in the file:

```sh
export LEETCODE_CSRF='<your-leetcode-csrf-token>'
export LEETCODE_SESSION='<your-leetcode-session-key>'
export LEETCODE_SITE='leetcode.com'   # or 'leetcode.cn'
```

`cookies.site` must still be present in `leetcode.toml` (otherwise config parsing fails), but `LEETCODE_SITE` overrides it at runtime.

## Cloudflare challenges

LeetCode fronts the judge endpoints with Cloudflare, which scores every request and answers the ones that look automated with a challenge page instead of a result. leetctl gets past the check by looking like the browser it borrows cookies from:

- it reproduces Chrome's **TLS and HTTP/2 fingerprint**, down to the extension order and the HTTP/2 settings, rather than the shape a generic Rust HTTP client would send;
- it sends Chrome's **`User-Agent`**, with the major version read from the installed Chrome;
- it forwards Chrome's **`cf_clearance`** cookie, Cloudflare's record that this machine already passed a check.

The three go together: Cloudflare ties `cf_clearance` to the agent string that earned it, and weighs both against the fingerprint. That is also why the **manual** and **environment variable** setups above can still be challenged — they carry the identity cookies but no `cf_clearance`.

If you do see the challenge error, open <https://leetcode.com> in Chrome, let the page settle, and retry. Waiting does not help on its own: `__cf_bm` expires after about half an hour of no traffic, so an idle session is *more* likely to be challenged than a busy one.

## leetcode.com vs leetcode.cn

`site` accepts exactly two values: `leetcode.com` or `leetcode.cn` (anything else is rejected). Choosing `leetcode.cn` switches every API endpoint to the China site — set it via the config field or `LEETCODE_SITE`.
