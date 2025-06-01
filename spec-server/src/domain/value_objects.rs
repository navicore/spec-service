use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpecName(String);

impl SpecName {
    pub fn new(name: String) -> Result<Self, ValidationError> {
        if name.is_empty() {
            return Err(ValidationError::EmptyName);
        }
        if name.len() > 255 {
            return Err(ValidationError::NameTooLong);
        }
        if !name.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.') {
            return Err(ValidationError::InvalidCharacters);
        }
        Ok(Self(name))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SpecName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpecContent(String);

impl SpecContent {
    pub fn new(content: String) -> Result<Self, ValidationError> {
        if content.is_empty() {
            return Err(ValidationError::EmptyContent);
        }
        if content.len() > 2048 {
            return Err(ValidationError::ContentTooLarge);
        }
        // Validate YAML
        serde_yaml::from_str::<serde_yaml::Value>(&content)
            .map_err(|_| ValidationError::InvalidYaml)?;
        Ok(Self(content))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Version(u32);

impl Version {
    pub fn new(version: u32) -> Self {
        Self(version)
    }

    pub fn initial() -> Self {
        Self(1)
    }

    pub fn increment(&self) -> Self {
        Self(self.0 + 1)
    }

    pub fn as_u32(&self) -> u32 {
        self.0
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    #[error("Name cannot be empty")]
    EmptyName,
    #[error("Name too long (max 255 characters)")]
    NameTooLong,
    #[error("Name contains invalid characters")]
    InvalidCharacters,
    #[error("Content cannot be empty")]
    EmptyContent,
    #[error("Content too large (max 2048 characters)")]
    ContentTooLarge,
    #[error("Invalid YAML content")]
    InvalidYaml,
}