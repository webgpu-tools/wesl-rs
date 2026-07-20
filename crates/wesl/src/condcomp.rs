use std::collections::HashMap;

/// Set the behavior for a feature flag during conditional translation.
///
/// * `Keep` means that the feature flag will be left as-is. This is useful for
///   incremental compilation, e.g. for generating shader variants
/// * `Error` means that unspecified feature flags will trigger a
///   [`CondCompError::UnexpectedFeatureFlag`].
///
/// Default is `Disable`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Feature {
    Enable,
    #[default]
    Disable,
    Keep,
    Error,
}

/// Toggle conditional compilation feature flags.
///
/// Feature flags set to `true` are enabled, and `false` are disabled. Feature flags not
/// present in `flags` are treated according to `default`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Features {
    pub default: Feature,
    pub flags: HashMap<String, Feature>,
}

impl From<bool> for Feature {
    fn from(value: bool) -> Self {
        if value {
            Feature::Enable
        } else {
            Feature::Disable
        }
    }
}

impl Features {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn add_feature(&mut self, name: impl ToString, value: impl Into<Feature>) {
        self.flags.insert(name.to_string(), value.into());
    }
}
