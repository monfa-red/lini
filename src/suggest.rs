//! Fuzzy "did you mean …?" suggestions [SPEC 20]: one Levenshtein and one
//! message shape for every error path that proposes a nearby name.

/// Levenshtein edit distance (two-row DP). Only ranks suggestions on the error
/// path, so it need not be fast.
pub fn edit_distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0usize; b.len() + 1];
    for (i, ca) in a.chars().enumerate() {
        cur[0] = i + 1;
        for (j, &cb) in b.iter().enumerate() {
            let sub = prev[j] + usize::from(ca != cb);
            cur[j + 1] = sub.min(prev[j + 1] + 1).min(cur[j] + 1);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

/// The at-most-`k` candidates closest to `input` by edit distance — nearest
/// first, no farther than a small typo (≤ 2 edits) away — with candidate order
/// breaking ties. Empty when nothing is close.
pub fn nearest<'a>(
    input: &str,
    candidates: impl IntoIterator<Item = &'a str>,
    k: usize,
) -> Vec<&'a str> {
    let mut near: Vec<(usize, usize, &str)> = candidates
        .into_iter()
        .enumerate()
        .map(|(i, c)| (edit_distance(input, c), i, c))
        .filter(|&(d, _, _)| d <= 2)
        .collect();
    near.sort_by_key(|&(d, i, _)| (d, i));
    near.into_iter().take(k).map(|(_, _, c)| c).collect()
}

/// The trailing "; did you mean 'a', 'b'?" clause for an error message — each
/// candidate single-quoted, comma-joined. Empty (no clause) when there are no
/// candidates, so it appends cleanly to any message.
pub fn did_you_mean<S: AsRef<str>>(candidates: &[S]) -> String {
    if candidates.is_empty() {
        return String::new();
    }
    let quoted: Vec<String> = candidates
        .iter()
        .map(|c| format!("'{}'", c.as_ref()))
        .collect();
    format!("; did you mean {}?", quoted.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distance_counts_edits() {
        assert_eq!(edit_distance("kitten", "sitting"), 3);
        assert_eq!(edit_distance("m9", "m8"), 1);
        assert_eq!(edit_distance("same", "same"), 0);
    }

    #[test]
    fn nearest_ranks_and_caps_at_a_typo() {
        let cands = ["portrait", "landscape", "a0"];
        assert_eq!(nearest("portrai", cands, 2), ["portrait"]);
        // Nothing within two edits → no suggestion.
        assert!(nearest("zzzzzz", cands, 2).is_empty());
    }

    #[test]
    fn nearest_breaks_ties_by_candidate_order() {
        // "a1".."a5" are all one edit from "a9"; the first wins.
        let cands = ["a0", "a1", "a2"];
        assert_eq!(nearest("a9", cands, 1), ["a0"]);
    }

    #[test]
    fn clause_is_empty_without_candidates() {
        assert_eq!(did_you_mean::<&str>(&[]), "");
        assert_eq!(did_you_mean(&["a", "b"]), "; did you mean 'a', 'b'?");
    }
}
