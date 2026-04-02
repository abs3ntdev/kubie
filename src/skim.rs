use std::borrow::Cow;
use std::io::Cursor;
use std::sync::Arc;

use anyhow::Result;
use skim::fuzzy_matcher::FuzzyMatcher;
use skim::prelude::{SkimItemReader, SkimItemSender, SkimOptionsBuilder};
use skim::{Skim, SkimItem};

use crate::settings::Fzf;

/// A skim item with an optional tag displayed after the name.
/// `text()` and `output()` return only the name so fuzzy matching
/// and the returned selection are clean context names.
/// `display()` appends the tag (e.g. `[doctl]`) for visual distinction.
pub struct TaggedItem {
    pub name: String,
    pub tag: Option<String>,
    /// Byte range of the name portion for fuzzy matching (excludes the tag).
    matching_range: (usize, usize),
}

impl TaggedItem {
    pub fn new(name: String, tag: Option<String>) -> Self {
        let range = (0, name.len());
        TaggedItem {
            name,
            tag,
            matching_range: range,
        }
    }
}

impl SkimItem for TaggedItem {
    fn text(&self) -> Cow<'_, str> {
        // Include the tag in the text so it's visible in the display.
        // Matching ranges are restricted to just the name portion so
        // fuzzy search only considers the context name.
        match &self.tag {
            Some(tag) => Cow::Owned(format!("{} [{}]", self.name, tag)),
            None => Cow::Borrowed(&self.name),
        }
    }

    fn get_matching_ranges(&self) -> Option<&[(usize, usize)]> {
        // Restrict fuzzy matching to just the name, excluding the tag.
        Some(std::slice::from_ref(&self.matching_range))
    }

    fn output(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.name)
    }
}

fn build_options(fzf: &Fzf) -> Result<skim::SkimOptions> {
    let mut options = SkimOptionsBuilder::default();

    options.no_multi(true).no_mouse(!fzf.mouse).reverse(fzf.reverse);

    if let Some(color) = &fzf.color {
        options.color(color.clone());
    }

    if fzf.ignore_case {
        options.case(skim::CaseMatching::Ignore);
    }

    if fzf.info_hidden {
        options.no_info(true);
    }

    if let Some(height) = &fzf.height {
        options.height(height.clone());
    }

    if let Some(prompt) = &fzf.prompt {
        options.prompt(prompt.clone());
    }

    options
        .build()
        .map_err(|e| anyhow::anyhow!("failed to build skim options: {}", e))
}

/// Run skim with the given items and return the selected item, if any
pub fn select(fzf: &Fzf, items: Vec<String>) -> Result<Option<String>> {
    let options = build_options(fzf)?;
    let reader = SkimItemReader::default();
    let rx = reader.of_bufread(Cursor::new(items.join("\n")));
    let output = Skim::run_with(options, Some(rx)).map_err(|e| anyhow::anyhow!("{e}"))?;

    if output.is_abort || output.selected_items.is_empty() {
        Ok(None)
    } else {
        Ok(Some(output.selected_items[0].output().to_string()))
    }
}

/// Run skim with tagged items displayed immediately, and a background closure
/// that can push additional tagged items into the picker as they are discovered.
///
/// The `bg` closure receives a `SkimItemSender` and should send any new items,
/// then return. The picker stays open and updates live as new items arrive.
pub fn select_with_bg<F>(fzf: &Fzf, initial: Vec<TaggedItem>, bg: F) -> Result<Option<String>>
where
    F: FnOnce(SkimItemSender) + Send + 'static,
{
    let options = build_options(fzf)?;
    let (tx, rx) = skim::prelude::bounded(50);

    // Send initial items immediately.
    if !initial.is_empty() {
        let items: Vec<Arc<dyn SkimItem>> = initial
            .into_iter()
            .map(|item| Arc::new(item) as Arc<dyn SkimItem>)
            .collect();
        let _ = tx.send(items);
    }

    // Spawn the background producer.
    let tx_bg = tx.clone();
    std::thread::spawn(move || {
        bg(tx_bg);
    });

    // Drop our sender so skim closes the channel once the bg thread finishes.
    drop(tx);

    let output = Skim::run_with(options, Some(rx)).map_err(|e| anyhow::anyhow!("{e}"))?;

    if output.is_abort || output.selected_items.is_empty() {
        Ok(None)
    } else {
        Ok(Some(output.selected_items[0].output().to_string()))
    }
}

/// Fuzzy match a query against a list of candidates. Returns the best match
/// if the score is above zero, or None if nothing matches.
pub fn fuzzy_match(query: &str, candidates: &[String]) -> Option<String> {
    let matcher = skim::fuzzy_matcher::skim::SkimMatcherV2::default();
    candidates
        .iter()
        .filter_map(|c| matcher.fuzzy_match(c, query).map(|score| (c, score)))
        .max_by_key(|(_, score)| *score)
        .map(|(c, _)| c.clone())
}
