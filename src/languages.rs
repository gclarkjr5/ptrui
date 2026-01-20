#[derive(Debug, Clone, Copy)]
pub struct Language {
    pub name: &'static str,
    pub code: &'static str,
}

pub const LANGUAGES: &[Language] = &[
    Language {
        name: "English",
        code: "EN",
    },
    Language {
        name: "Spanish",
        code: "ES",
    },
    Language {
        name: "French",
        code: "FR",
    },
    Language {
        name: "German",
        code: "DE",
    },
    Language {
        name: "Italian",
        code: "IT",
    },
    Language {
        name: "Portuguese",
        code: "PT",
    },
    Language {
        name: "Dutch",
        code: "NL",
    },
    Language {
        name: "Polish",
        code: "PL",
    },
    Language {
        name: "Russian",
        code: "RU",
    },
    Language {
        name: "Japanese",
        code: "JA",
    },
    Language {
        name: "Chinese",
        code: "ZH",
    },
    Language {
        name: "Korean",
        code: "KO",
    },
    Language {
        name: "Swedish",
        code: "SV",
    },
];

pub fn find_language_index(code: &str) -> Option<usize> {
    LANGUAGES
        .iter()
        .position(|language| language.code.eq_ignore_ascii_case(code))
}

pub fn filtered_language_indices(query: &str) -> Vec<usize> {
    if query.trim().is_empty() {
        return (0..LANGUAGES.len()).collect();
    }
    let mut matches: Vec<(usize, usize)> = Vec::new();
    for (index, language) in LANGUAGES.iter().enumerate() {
        let candidate = format!(
            "{} {}",
            language.name.to_ascii_lowercase(),
            language.code.to_ascii_lowercase()
        );
        if let Some(score) = fuzzy_score(query, &candidate) {
            matches.push((score, index));
        }
    }
    matches.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then_with(|| LANGUAGES[a.1].name.cmp(LANGUAGES[b.1].name))
    });
    matches.into_iter().map(|(_, index)| index).collect()
}

fn fuzzy_score(query: &str, candidate: &str) -> Option<usize> {
    let mut score = 0usize;
    let mut last_index = 0usize;
    let query_lower = query.to_ascii_lowercase();
    for needle in query_lower.chars() {
        if let Some(found) = candidate[last_index..].find(needle) {
            score += found;
            last_index += found + 1;
        } else {
            return None;
        }
    }
    Some(score)
}
