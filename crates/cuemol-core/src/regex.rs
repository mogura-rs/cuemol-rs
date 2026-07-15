use crate::{CueMolError, LString, Result};
use regex::RegexBuilder;
use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
pub struct LRegExpr {
    pattern: LString,
    subject: LString,
    captures: Vec<Option<LString>>,
    names: HashMap<String, usize>,
}

impl LRegExpr {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_pattern(pattern: impl Into<LString>) -> Self {
        Self {
            pattern: pattern.into(),
            ..Self::default()
        }
    }

    pub fn set_pattern(&mut self, pattern: impl Into<LString>) {
        self.pattern = pattern.into();
        self.captures.clear();
        self.names.clear();
    }

    pub fn setup(&mut self, pattern: impl Into<LString>) {
        self.set_pattern(pattern);
    }

    pub fn get_pattern(&self) -> &LString {
        &self.pattern
    }

    pub fn pattern(&self) -> &LString {
        self.get_pattern()
    }

    pub fn subject(&self) -> &LString {
        &self.subject
    }

    pub fn get_subj(&self) -> &LString {
        self.subject()
    }

    pub fn to_script_string(&self) -> LString {
        self.pattern.clone()
    }

    #[allow(clippy::inherent_to_string)]
    pub fn to_string(&self) -> String {
        self.pattern.as_str().to_string()
    }

    pub fn match_text(&mut self, subject: impl Into<LString>) -> Result<bool> {
        self.match_impl(subject.into(), false)
    }

    pub fn r#match(&mut self, subject: impl Into<LString>) -> Result<bool> {
        self.match_text(subject)
    }

    pub fn match_ignore_case(&mut self, subject: impl Into<LString>) -> Result<bool> {
        self.match_impl(subject.into(), true)
    }

    pub fn get_substr_count(&self) -> usize {
        self.captures.len()
    }

    pub fn size(&self) -> usize {
        self.get_substr_count()
    }

    pub fn get_substr(&self, index: usize) -> Result<LString> {
        self.captures
            .get(index)
            .and_then(Clone::clone)
            .ok_or_else(|| CueMolError::invalid_argument("substring not found"))
    }

    pub fn at(&self, index: usize) -> Result<LString> {
        self.get_substr(index)
    }

    pub fn get_named_substr(&self, name: impl AsRef<str>) -> Result<LString> {
        let index = self
            .names
            .get(name.as_ref())
            .copied()
            .ok_or_else(|| CueMolError::invalid_argument("substring not found"))?;
        self.get_substr(index)
    }

    fn match_impl(&mut self, subject: LString, ignore_case: bool) -> Result<bool> {
        let expression = RegexBuilder::new(self.pattern.as_str())
            .case_insensitive(ignore_case)
            .build()
            .map_err(|error| CueMolError::invalid_argument(error.to_string()))?;
        self.names = expression
            .capture_names()
            .enumerate()
            .filter_map(|(index, name)| name.map(|name| (name.to_string(), index)))
            .collect();
        self.subject = subject;

        let Some(captures) = expression.captures(self.subject.as_str()) else {
            self.captures.clear();
            return Ok(false);
        };
        self.captures = (0..captures.len())
            .map(|index| {
                captures
                    .get(index)
                    .map(|value| LString::from(value.as_str()))
            })
            .collect();
        Ok(true)
    }
}
