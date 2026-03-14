use super::*;

impl App {

    pub(super) fn fuzzy_score(query: &str, text: &str) -> Option<i32> {
        if query.is_empty() {
            return Some(0);
        }

        let q = query.to_lowercase();
        let t = text.to_lowercase();

        let mut score: i32 = 0;
        let mut last_match: Option<usize> = None;
        let mut pos = 0usize;

        for ch in q.chars() {
            if let Some(found) = t[pos..].find(ch) {
                let idx = pos + found;
                score += 10;
                if let Some(prev) = last_match {
                    if idx == prev + 1 {
                        score += 15; // contiguous bonus
                    } else {
                        score -= (idx.saturating_sub(prev) as i32).min(10);
                    }
                } else {
                    score -= idx.min(15) as i32; // earlier is better
                }
                last_match = Some(idx);
                pos = idx + ch.len_utf8();
            } else {
                return None;
            }
        }

        Some(score)
    }

    pub(super) fn update_search_results(&mut self) {
        let q = self.search_query.trim();
        if q.is_empty() {
            self.search_results.clear();
            self.search_selected = 0;
            return;
        }

        let mut scored: Vec<(i32, String)> = Vec::new();
        for s in &self.sessions {
            let hay = format!(
                "{} {} {}",
                s.title,
                s.group_path,
                s.project_path.to_string_lossy()
            );
            if let Some(score) = Self::fuzzy_score(q, &hay) {
                scored.push((score, s.id.clone()));
            }
        }

        scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        self.search_results = scored.into_iter().map(|(_, id)| id).take(50).collect();
        if self.search_selected >= self.search_results.len() {
            self.search_selected = 0;
        }
    }
}
