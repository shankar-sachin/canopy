//! Small subsequence fuzzy matcher used by filters and the palette.

/// Score `needle` against `hay`; `None` if not all chars appear in order.
/// Higher is better. Consecutive runs and word starts are rewarded.
pub fn score(needle: &str, hay: &str) -> Option<i64> {
    if needle.is_empty() {
        return Some(0);
    }
    let hay_chars: Vec<char> = hay.chars().collect();
    let hay_lower: Vec<char> = hay.to_lowercase().chars().collect();
    let smart_case = needle.chars().any(|c| c.is_uppercase());
    let mut score = 0i64;
    let mut hi = 0usize;
    let mut prev: Option<usize> = None;
    for n in needle.chars() {
        let found = (hi..hay_lower.len()).find(|&i| {
            if smart_case {
                hay_chars.get(i) == Some(&n)
            } else {
                n.to_lowercase().next() == Some(hay_lower[i])
            }
        })?;
        score += 10;
        if prev == Some(found.wrapping_sub(1)) {
            score += 15;
        }
        if found == 0 || !hay_chars.get(found - 1).is_some_and(|c| c.is_alphanumeric()) {
            score += 10;
        }
        score -= (found - hi) as i64;
        prev = Some(found);
        hi = found + 1;
    }
    Some(score - hay_lower.len() as i64 / 10)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_subsequence() {
        assert!(score("stg", "stage/unstage").is_some());
        assert!(score("xyz", "stage").is_none());
        assert!(score("psh", "push").is_some());
    }

    #[test]
    fn prefers_word_starts_and_runs() {
        let a = score("push", "push").unwrap();
        let b = score("push", "p u s h x").unwrap();
        assert!(a > b);
    }

    #[test]
    fn smart_case() {
        assert!(score("Fix", "fix bug").is_none());
        assert!(score("fix", "Fix bug").is_some());
    }
}
