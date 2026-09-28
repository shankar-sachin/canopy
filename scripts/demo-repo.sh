#!/bin/sh
# Make a small demo repository for recordings and screenshots: a few commits
# by two people, a feature branch, a tag, and uncommitted edits to a Rust
# file (so the diff has something to highlight). Used by demo.tape.
#
#   scripts/demo-repo.sh /tmp/acme-app
set -eu
dir=${1:?usage: scripts/demo-repo.sh <folder>}
rm -rf "$dir"
mkdir -p "$dir/src"
cd "$dir"
git init -q -b main
git config user.name "Ada Lovelace"
git config user.email ada@example.com
git config commit.gpgsign false
commit() { GIT_AUTHOR_NAME="$1" GIT_AUTHOR_EMAIL="$2" git commit -q -m "$3"; }

cat > README.md <<'EOF'
# acme-app

Counts the words in a file.
EOF
cat > src/main.rs <<'EOF'
use std::collections::HashMap;

/// Count how often each word appears.
fn count_words(text: &str) -> HashMap<String, usize> {
    let mut counts = HashMap::new();
    for word in text.split_whitespace() {
        *counts.entry(word.to_string()).or_insert(0) += 1;
    }
    counts
}

fn main() {
    let text = std::fs::read_to_string("input.txt").unwrap();
    let counts = count_words(&text);
    println!("{} different words", counts.len());
}
EOF
git add . && commit "Ada Lovelace" ada@example.com "Count words in a file"

printf '/target\n' > .gitignore
git add . && commit "Grace Hopper" grace@example.com "Ignore build output"
git tag v0.1.0

git switch -q -c feature/top-words
cat >> src/main.rs <<'EOF'

/// The `n` most common words, most common first.
fn top_words(counts: &HashMap<String, usize>, n: usize) -> Vec<(&String, &usize)> {
    let mut all: Vec<_> = counts.iter().collect();
    all.sort_by(|a, b| b.1.cmp(a.1));
    all.truncate(n);
    all
}
EOF
git add . && commit "Grace Hopper" grace@example.com "Add top_words"
git switch -q main

# Uncommitted: a better word count, not staged yet.
cat > src/main.rs <<'EOF'
use std::collections::HashMap;

/// Count how often each word appears, ignoring case and punctuation.
fn count_words(text: &str) -> HashMap<String, usize> {
    let mut counts = HashMap::new();
    for word in text.split_whitespace().map(normalize).filter(|w| !w.is_empty()) {
        *counts.entry(word).or_insert(0) += 1;
    }
    counts
}

/// "Hello," and "hello" are the same word.
fn normalize(word: &str) -> String {
    word.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase()
}

fn main() {
    let path = std::env::args().nth(1).unwrap_or_else(|| "input.txt".into());
    let text = std::fs::read_to_string(&path).expect("can't read the file");
    let counts = count_words(&text);
    println!("{} different words in {path}", counts.len());
}
EOF
echo "demo-repo: $dir"
