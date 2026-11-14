//! Tiny fuzzy matcher: case-insensitive subsequence with a score that favours
//! consecutive runs and matches at word starts. Pure, no allocation per char.

/// Scores `candidate` against `query`; `None` when `query` is not a subsequence.
/// An empty query matches everything with score 0.
pub fn score(query: &str, candidate: &str) -> Option<u32> {
    let mut total = 0;
    let mut prev_matched = false;
    let mut prev_char: Option<char> = None;
    let mut q = query.chars().flat_map(char::to_lowercase).peekable();
    for c in candidate.chars() {
        let Some(&want) = q.peek() else { break };
        let lc = c.to_lowercase().next().unwrap_or(c);
        if lc == want {
            q.next();
            total += 1;
            if prev_matched {
                total += 4;
            }
            if prev_char.is_none_or(|p| !p.is_alphanumeric()) {
                total += 2;
            }
            prev_matched = true;
        } else {
            prev_matched = false;
        }
        prev_char = Some(c);
    }
    q.peek().is_none().then_some(total)
}

/// Indices of `items` matching `query`, best score first; ties keep input order.
pub fn filter<'a>(query: &str, items: impl IntoIterator<Item = &'a str>) -> Vec<usize> {
    let mut hits: Vec<(usize, u32)> = items
        .into_iter()
        .enumerate()
        .filter_map(|(i, s)| score(query, s).map(|sc| (i, sc)))
        .collect();
    hits.sort_by(|a, b| b.1.cmp(&a.1)); // stable: ties stay in order
    hits.into_iter().map(|(i, _)| i).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_query_matches_all_in_order() {
        assert_eq!(score("", "anything"), Some(0));
        assert_eq!(filter("", ["b", "a"]), vec![0, 1]);
    }

    #[test]
    fn subsequence_case_insensitive() {
        assert!(score("mrs", "src/MAIN.rs").is_some());
        assert!(score("MAIN", "src/main.rs").is_some());
        assert_eq!(score("xyz", "src/main.rs"), None);
        assert_eq!(score("sm", "ms"), None, "order matters");
    }

    #[test]
    fn consecutive_and_word_start_rank_higher() {
        assert!(score("main", "src/main.rs") > score("main", "m_a_i_n"));
        assert!(score("p", "add parser") > score("p", "help"));
    }

    #[test]
    fn filter_sorts_by_score_then_input_order() {
        let items = ["src/new.rs", "notes.txt", "src/main.rs"];
        assert_eq!(filter("main", items), vec![2]);
        assert_eq!(filter("no", items), vec![1]);
        assert_eq!(filter("ma", ["m_a", "main"]), vec![1, 0]);
        assert_eq!(filter("n", items), vec![0, 1, 2], "ties keep order");
        assert!(filter("zz", items).is_empty());
    }
}
