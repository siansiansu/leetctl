# Records docs/public/demo.gif from `leetctl tui`, run from the repo root:
#
#   make demo          # builds the release binary, puts it on PATH, runs vhs
#
# The recording uses the real cache in ~/.leetcode — the TUI is read-only in
# every key this tape presses, so nothing here submits code or changes a deck.
# Nothing on screen carries account data: the panels show problem counts and
# the local review deck, never the cookie file or a username.
#
# Shrink the result afterwards with:
#   gifsicle -O3 --lossy=40 --colors 96 docs/public/demo.gif -o docs/public/demo.gif
#
# The recording ends on the help page — never back at a shell prompt.

Output docs/public/demo.gif

Set Shell bash
Set FontSize 14
Set Width 1400
Set Height 800
Set Padding 10
Set Framerate 25
Set TypingSpeed 55ms
Set WaitTimeout 30s

# Hidden setup: a bare prompt, so no personal shell theme reaches a frame.
Hide
Type "PS1='$ '"
Enter
Type "clear"
Enter

# Launch while hidden so the first visible frame is already the table.
Type "leetctl tui"
Enter
Sleep 3s
Show

# --- the stats panel and the table ---------------------------------------
Sleep 2500ms
Down 6
Sleep 1200ms

# --- search: tokens are ANDed, !token excludes ---------------------------
Type "/"
Sleep 500ms
Type "binary tree"
Enter
Sleep 2s
Type "/"
Sleep 400ms
Type "binary tree !path"
Enter
Sleep 2s
Escape
Sleep 800ms

# --- a curated set, compiled into the binary -----------------------------
Type "s"
Sleep 1200ms
Down 2
Sleep 800ms
Enter
Sleep 2s

# --- difficulty cycles all -> easy -> medium -> hard ---------------------
Type "d"
Sleep 1200ms
Type "d"
Sleep 1800ms

# --- unsolved only: the bars and counts follow the pool ------------------
Type "u"
Sleep 2200ms
Type "u"
Sleep 800ms

# --- the review deck ------------------------------------------------------
Type "r"
Sleep 2200ms
Type "r"
Sleep 800ms
Escape
Sleep 800ms

# --- a problem description -----------------------------------------------
Down 3
Enter
Sleep 2500ms
Down 8
Sleep 1500ms
Escape
Sleep 800ms

# --- help -----------------------------------------------------------------
Type "?"
Sleep 3s
