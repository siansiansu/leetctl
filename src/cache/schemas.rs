//! Leetcode data schemas
table! {
    problems(id) {
        category -> Text,
        fid -> Integer,
        id -> Integer,
        level -> Integer,
        locked -> Bool,
        name -> Text,
        percent -> Float,
        slug -> Text,
        starred -> Bool,
        status -> Text,
        desc -> Text,
    }
}

// Tags
table! {
    tags(tag) {
        tag -> Text,
        refs -> Text,
    }
}

// The spaced-repetition deck. Keyed on the *frontend* id, the number `leetctl pick <id>` takes —
// not on `problems.id`, which is LeetCode's internal id.
table! {
    reviews(fid) {
        fid -> Integer,
        ease -> Float,
        interval_days -> Integer,
        repetitions -> Integer,
        lapses -> Integer,
        due_day -> Integer,
        last_day -> Integer,
    }
}
